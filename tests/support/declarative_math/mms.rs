//! Real retained MMS callbacks: construction/parsing is outside update timing.
use super::native::{CHAIN_LENGTH, Input, Kernel};
use meow_meow_script::{
    CallbackHandle, Host, HostError, HostRequest, HostResponse, Runtime, Session, Value,
};

#[derive(Clone, Copy, Debug)]
pub enum Style {
    ScalarLocals,
    Arrays,
    ArrayHelpers,
}
pub const STYLES: [Style; 3] = [Style::ScalarLocals, Style::Arrays, Style::ArrayHelpers];
impl Style {
    pub fn name(self) -> &'static str {
        match self {
            Self::ScalarLocals => "mms_scalar_locals",
            Self::Arrays => "mms_arrays",
            Self::ArrayHelpers => "mms_array_helpers",
        }
    }
}

#[derive(Default)]
pub struct CaptureHost {
    callback: Option<CallbackHandle>,
}
impl Host for CaptureHost {
    fn dispatch(&mut self, request: HostRequest) -> Result<HostResponse, HostError> {
        match request {
            HostRequest::RegisterSignalHandlerByName { callback, .. }
            | HostRequest::RegisterSignalHandler { callback, .. } => {
                self.callback = Some(callback);
                Ok(HostResponse::Unit)
            }
            other => Err(HostError::unsupported(other.operation_name())),
        }
    }
}

pub struct MmsKernel {
    session: Session<CaptureHost>,
    callback: CallbackHandle,
}
impl MmsKernel {
    pub fn new(runtime: &Runtime, source: &str) -> Self {
        let mut session = runtime.session(CaptureHost::default());
        session
            .eval(source)
            .unwrap_or_else(|error| panic!("{error}\n{source}"));
        let callback = session
            .host()
            .callback
            .expect("fixture registered a callback");
        Self { session, callback }
    }

    pub fn evaluate(&mut self, arguments: Vec<Value>) -> super::BatchOutput {
        let value = self
            .session
            .invoke_callback(self.callback, arguments)
            .expect("MMS math callback");
        let Value::Array(mut values) = value else {
            panic!("expected checksum/last result")
        };
        assert_eq!(values.len(), 2);
        let Value::Array(last) = values.pop().unwrap() else {
            panic!("expected numeric result")
        };
        let Value::Number(checksum) = values.pop().unwrap() else {
            panic!("expected checksum")
        };
        assert_eq!(last.len(), 4);
        super::BatchOutput {
            checksum,
            last: std::array::from_fn(|i| match last[i] {
                Value::Number(v) => v,
                _ => panic!("numeric result"),
            }),
        }
    }
}

/// The conversion is measured separately, never hidden in native math timing.
pub fn arguments(inputs: &[Input]) -> Vec<Value> {
    vec![Value::Array(
        inputs
            .iter()
            .map(|input| {
                Value::Array(
                    input
                        .a
                        .into_iter()
                        .chain(input.b)
                        .chain(input.q)
                        .chain([input.t])
                        .map(Value::Number)
                        .collect(),
                )
            })
            .collect(),
    )]
}

const ROTATE_HELPER: &str = r#"
let rotate = fn(q, v) {
    let tx = 2.0 * (q[1] * v[2] - q[2] * v[1])
    let ty = 2.0 * (q[2] * v[0] - q[0] * v[2])
    let tz = 2.0 * (q[0] * v[1] - q[1] * v[0])
    return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty),
            v[1] + q[3] * ty + (q[2] * tx - q[0] * tz),
            v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]
}
"#;
const ADD_HELPER: &str = "let add = fn(a, b) { return [a[0]+b[0], a[1]+b[1], a[2]+b[2]] }\n";
const SUBTRACT_HELPER: &str =
    "let subtract = fn(a, b) { return [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }\n";
const SCALE_HELPER: &str = "let scale = fn(v, s) { return [v[0]*s, v[1]*s, v[2]*s] }\n";
const COMPOSE_HELPER: &str = r#"
let compose = fn(a, b) {
    return [a[3]*b[0]+a[0]*b[3]+a[1]*b[2]-a[2]*b[1],
            a[3]*b[1]-a[0]*b[2]+a[1]*b[3]+a[2]*b[0],
            a[3]*b[2]+a[0]*b[1]-a[1]*b[0]+a[2]*b[3],
            a[3]*b[3]-a[0]*b[0]-a[1]*b[1]-a[2]*b[2]]
}
"#;

/// Generate ordinary MMS statements, not new declarative component syntax.
/// Chain bodies are unrolled identically to the native graphs: no loop-dispatch
/// difference between a 16-stage Rust plan and a script-controlled math loop.
pub fn source(kernel: Kernel, style: Style) -> String {
    let scalar = matches!(style, Style::ScalarLocals);
    let helpers = matches!(style, Style::ArrayHelpers);
    let mut prefix = String::new();
    let mut body = String::new();
    if helpers {
        if matches!(
            kernel,
            Kernel::QuaternionRotate | Kernel::BodyOffset | Kernel::SharedRotation
        ) {
            prefix.push_str(ROTATE_HELPER);
        }
        if matches!(kernel, Kernel::VectorChain | Kernel::SharedRotation) {
            prefix.push_str(ADD_HELPER);
            prefix.push_str(SCALE_HELPER);
        }
        if matches!(kernel, Kernel::BodyOffset) {
            prefix.push_str(SUBTRACT_HELPER);
        }
        if matches!(kernel, Kernel::QuaternionCompose) {
            prefix.push_str(COMPOSE_HELPER);
        }
    }
    let uses_a = !matches!(kernel, Kernel::ScalarChain | Kernel::QuaternionCompose);
    let uses_b = matches!(
        kernel,
        Kernel::VectorChain | Kernel::BodyOffset | Kernel::SharedRotation
    );
    let uses_q = !matches!(kernel, Kernel::ScalarChain | Kernel::VectorChain);
    for (used, name, start, length) in [
        (uses_a, "a", 0, 3),
        (uses_b, "b", 3, 3),
        (uses_q, "q", 6, 4),
    ] {
        if !used {
            continue;
        }
        if scalar {
            for i in 0..length {
                body.push_str(&format!("let {name}{i} = row[{}]\n", start + i));
            }
        } else {
            let fields = (start..start + length)
                .map(|i| format!("row[{i}]"))
                .collect::<Vec<_>>()
                .join(", ");
            body.push_str(&format!("let {name} = [{fields}]\n"));
        }
    }
    if matches!(
        kernel,
        Kernel::ScalarChain | Kernel::VectorChain | Kernel::SharedRotation
    ) {
        body.push_str("let amount = row[10]\n");
    }
    match kernel {
        Kernel::ScalarChain => {
            body.push_str("let s = amount\n");
            for _ in 0..CHAIN_LENGTH {
                body.push_str("s = Math.clamp(s * 1.0001 + 0.001, -1.0, 1.0)\n");
            }
            body.push_str("last = [s, 0.0, 0.0, 0.0]\n");
        }
        Kernel::VectorChain => {
            if scalar {
                body.push_str("let x=a0\nlet y=a1\nlet z=a2\n");
                for _ in 0..CHAIN_LENGTH {
                    body.push_str("x=x+b0*amount\ny=y+b1*amount\nz=z+b2*amount\n");
                }
                body.push_str("last=[x,y,z,0.0]\n");
            } else {
                body.push_str("let v=a\n");
                for _ in 0..CHAIN_LENGTH {
                    body.push_str(if helpers { "v=add(v,scale(b,amount))\n" } else {
                        "let scaled=[b[0]*amount,b[1]*amount,b[2]*amount]\nv=[v[0]+scaled[0],v[1]+scaled[1],v[2]+scaled[2]]\n"
                    });
                }
                body.push_str("last=[v[0],v[1],v[2],0.0]\n");
            }
        }
        Kernel::QuaternionCompose => {
            if scalar {
                body.push_str(
                    "let r0=q3*0.0+q0*0.9887710779360422+q1*0.0-q2*0.14943813247359922\n\
                     let r1=q3*0.14943813247359922-q0*0.0+q1*0.9887710779360422+q2*0.0\n\
                     let r2=q3*0.0+q0*0.14943813247359922-q1*0.0+q2*0.9887710779360422\n\
                     let r3=q3*0.9887710779360422-q0*0.0-q1*0.14943813247359922-q2*0.0\n\
                     last=[r0,r1,r2,r3]\n",
                );
            } else {
                body.push_str("let rest=[0.0,0.14943813247359922,0.0,0.9887710779360422]\n");
                if helpers {
                    body.push_str("last=compose(q,rest)\n");
                } else {
                    body.push_str(
                        "last=[q[3]*rest[0]+q[0]*rest[3]+q[1]*rest[2]-q[2]*rest[1],\
                           q[3]*rest[1]-q[0]*rest[2]+q[1]*rest[3]+q[2]*rest[0],\
                           q[3]*rest[2]+q[0]*rest[1]-q[1]*rest[0]+q[2]*rest[3],\
                           q[3]*rest[3]-q[0]*rest[0]-q[1]*rest[1]-q[2]*rest[2]]\n",
                    );
                }
            }
        }
        Kernel::QuaternionRotate | Kernel::BodyOffset | Kernel::SharedRotation => {
            if scalar {
                let v = if matches!(kernel, Kernel::BodyOffset) {
                    ["0.0", "0.12", "0.0"]
                } else {
                    ["a0", "a1", "a2"]
                };
                body.push_str(&format!(
                    "let tx=2.0*(q1*{}-q2*{})\nlet ty=2.0*(q2*{}-q0*{})\nlet tz=2.0*(q0*{}-q1*{})\n\
                     let rx={}+q3*tx+(q1*tz-q2*ty)\nlet ry={}+q3*ty+(q2*tx-q0*tz)\nlet rz={}+q3*tz+(q0*ty-q1*tx)\n",
                    v[2],v[1],v[0],v[2],v[1],v[0],v[0],v[1],v[2]));
                body.push_str(match kernel {
                    Kernel::BodyOffset => "last=[a0-b0-rx,a1-b1-ry,a2-b2-rz,0.0]\n",
                    Kernel::SharedRotation => {
                        "last=[(rx+b0)+rx*amount,(ry+b1)+ry*amount,(rz+b2)+rz*amount,0.0]\n"
                    }
                    _ => "last=[rx,ry,rz,0.0]\n",
                });
            } else {
                let v = if matches!(kernel, Kernel::BodyOffset) {
                    body.push_str("let neck=[0.0,0.12,0.0]\n");
                    "neck"
                } else {
                    "a"
                };
                if helpers {
                    body.push_str(&format!("let r=rotate(q,{v})\n"));
                } else {
                    body.push_str(&format!(
                    "let temp=[2.0*(q[1]*{v}[2]-q[2]*{v}[1]),2.0*(q[2]*{v}[0]-q[0]*{v}[2]),2.0*(q[0]*{v}[1]-q[1]*{v}[0])]\n\
                     let r=[{v}[0]+q[3]*temp[0]+(q[1]*temp[2]-q[2]*temp[1]),\
                            {v}[1]+q[3]*temp[1]+(q[2]*temp[0]-q[0]*temp[2]),\
                            {v}[2]+q[3]*temp[2]+(q[0]*temp[1]-q[1]*temp[0])]\n"));
                }
                match kernel {
                    Kernel::BodyOffset if helpers => body.push_str("let result=subtract(subtract(a,b),r)\n"),
                    Kernel::BodyOffset => body.push_str("let base=[a[0]-b[0],a[1]-b[1],a[2]-b[2]]\nlet result=[base[0]-r[0],base[1]-r[1],base[2]-r[2]]\n"),
                    Kernel::SharedRotation if helpers => body.push_str("let result=add(add(r,b),scale(r,amount))\n"),
                    Kernel::SharedRotation => body.push_str("let plus=[r[0]+b[0],r[1]+b[1],r[2]+b[2]]\nlet scaled=[r[0]*amount,r[1]*amount,r[2]*amount]\nlet result=[plus[0]+scaled[0],plus[1]+scaled[1],plus[2]+scaled[2]]\n"),
                    _ => body.push_str("let result=r\n"),
                }
                body.push_str("last=[result[0],result[1],result[2],0.0]\n");
            }
        }
    }
    format!(
        "{prefix}\non_global(\"MathProfile\", fn(rows) {{\n\
        let checksum=0.0\nlet last=[0.0,0.0,0.0,0.0]\n\
        for row in rows {{\n{body}\n\
            checksum=checksum+last[0]+last[1]*3.0+last[2]*5.0+last[3]*7.0\n\
        }}\nreturn [checksum,last]\n}})\n"
    )
}
