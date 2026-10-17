//! Benchmark-only implementations. These are NOT production math components.
//! f64 deliberately matches MMS Number, avoiding a precision-width comparison.

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub a: [f64; 3],
    pub b: [f64; 3],
    pub q: [f64; 4],
    pub t: f64,
}

#[derive(Clone, Copy, Debug)]
pub enum Output {
    Scalar(f64),
    Vector([f64; 3]),
    Quaternion([f64; 4]),
}

impl Output {
    pub fn components(self) -> [f64; 4] {
        match self {
            Self::Scalar(s) => [s, 0.0, 0.0, 0.0],
            Self::Vector([x, y, z]) => [x, y, z, 0.0],
            Self::Quaternion(q) => q,
        }
    }

    pub fn checksum(self) -> f64 {
        let [x, y, z, w] = self.components();
        x + y * 3.0 + z * 5.0 + w * 7.0
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Kernel {
    ScalarChain,
    VectorChain,
    QuaternionRotate,
    QuaternionCompose,
    BodyOffset,
    SharedRotation,
}

pub const KERNELS: [Kernel; 6] = [
    Kernel::ScalarChain,
    Kernel::VectorChain,
    Kernel::QuaternionRotate,
    Kernel::QuaternionCompose,
    Kernel::BodyOffset,
    Kernel::SharedRotation,
];

pub const CHAIN_LENGTH: usize = 16;
pub const REST: [f64; 3] = [0.0, 0.12, 0.0];
pub const REST_Q: [f64; 4] = [0.0, 0.14943813247359922, 0.0, 0.9887710779360422];

impl Kernel {
    pub fn name(self) -> &'static str {
        match self {
            Self::ScalarChain => "scalar_chain",
            Self::VectorChain => "vector_chain",
            Self::QuaternionRotate => "quaternion_rotate",
            Self::QuaternionCompose => "quaternion_compose",
            Self::BodyOffset => "body_offset",
            Self::SharedRotation => "shared_rotation",
        }
    }
}

pub fn fixture(frame: usize, subject: usize) -> Input {
    let phase = ((frame * 17 + subject * 13) % 257) as f64 / 257.0;
    let angle = phase * 2.4 - 1.2;
    // General, unit quaternion with all three axes represented.
    let s = (angle * 0.5).sin() / 14.0f64.sqrt();
    Input {
        a: [phase - 0.5, 1.6 + phase * 0.2, phase * 0.7],
        b: [0.03 + phase * 0.01, -0.2, 0.15 - phase * 0.1],
        q: [s, s * 2.0, s * 3.0, (angle * 0.5).cos()],
        t: phase - 0.5,
    }
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn scale(v: [f64; 3], s: f64) -> [f64; 3] {
    v.map(|x| x * s)
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub fn rotate(q: [f64; 4], v: [f64; 3]) -> [f64; 3] {
    let xyz = [q[0], q[1], q[2]];
    let t = scale(cross(xyz, v), 2.0);
    add(add(v, scale(t, q[3])), cross(xyz, t))
}
pub fn compose(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

/// Adapter boundary: the future real MathGraph backend must implement this
/// against the same fixtures, not replace reference/prototype measurements.
pub trait NativeBackend {
    fn evaluate(&mut self, input: Input) -> Output;
}

pub struct Direct(pub Kernel);
impl NativeBackend for Direct {
    fn evaluate(&mut self, input: Input) -> Output {
        let Input { a, b, q, t } = input;
        match self.0 {
            Kernel::ScalarChain => {
                let mut s = t;
                for _ in 0..CHAIN_LENGTH {
                    s = (s * 1.0001 + 0.001).clamp(-1.0, 1.0);
                }
                Output::Scalar(s)
            }
            Kernel::VectorChain => {
                let mut v = a;
                for _ in 0..CHAIN_LENGTH {
                    v = add(v, scale(b, t));
                }
                Output::Vector(v)
            }
            Kernel::QuaternionRotate => Output::Vector(rotate(q, a)),
            Kernel::QuaternionCompose => Output::Quaternion(compose(q, REST_Q)),
            Kernel::BodyOffset => Output::Vector(sub(sub(a, b), rotate(q, REST))),
            Kernel::SharedRotation => {
                let rotated = rotate(q, a);
                Output::Vector(add(add(rotated, b), scale(rotated, t)))
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Node {
    A,
    B,
    Q,
    T,
    Constant(Output),
    ScalarAdd(usize, usize),
    ScalarMultiply(usize, usize),
    Clamp(usize),
    VectorAdd(usize, usize),
    VectorSubtract(usize, usize),
    VectorScale(usize, usize),
    Rotate(usize, usize),
    Compose(usize, usize),
}

pub struct Graph {
    nodes: Vec<Node>,
    memo: Vec<Option<Output>>,
    output: usize,
    pub evaluations: usize,
}

impl Graph {
    pub fn new(kernel: Kernel) -> Self {
        let mut graph = Self {
            nodes: vec![Node::A, Node::B, Node::Q, Node::T],
            memo: Vec::new(),
            output: 0,
            evaluations: 0,
        };
        graph.output = match kernel {
            Kernel::ScalarChain => {
                let factor = graph.push(Node::Constant(Output::Scalar(1.0001)));
                let offset = graph.push(Node::Constant(Output::Scalar(0.001)));
                let mut value = 3;
                for _ in 0..CHAIN_LENGTH {
                    let product = graph.push(Node::ScalarMultiply(value, factor));
                    let sum = graph.push(Node::ScalarAdd(product, offset));
                    value = graph.push(Node::Clamp(sum));
                }
                value
            }
            Kernel::VectorChain => {
                let mut value = 0;
                for _ in 0..CHAIN_LENGTH {
                    let scaled = graph.push(Node::VectorScale(1, 3));
                    value = graph.push(Node::VectorAdd(value, scaled));
                }
                value
            }
            Kernel::QuaternionRotate => graph.push(Node::Rotate(2, 0)),
            Kernel::QuaternionCompose => {
                let rest = graph.push(Node::Constant(Output::Quaternion(REST_Q)));
                graph.push(Node::Compose(2, rest))
            }
            Kernel::BodyOffset => {
                let rest = graph.push(Node::Constant(Output::Vector(REST)));
                let arc = graph.push(Node::Rotate(2, rest));
                let base = graph.push(Node::VectorSubtract(0, 1));
                graph.push(Node::VectorSubtract(base, arc))
            }
            Kernel::SharedRotation => {
                let rotated = graph.push(Node::Rotate(2, 0));
                let plus = graph.push(Node::VectorAdd(rotated, 1));
                let scaled = graph.push(Node::VectorScale(rotated, 3));
                graph.push(Node::VectorAdd(plus, scaled))
            }
        };
        graph.memo.resize(graph.nodes.len(), None);
        graph
    }

    fn push(&mut self, node: Node) -> usize {
        let id = self.nodes.len();
        self.nodes.push(node);
        id
    }

    fn eval_node(&mut self, id: usize, input: Input) -> Output {
        if let Some(value) = self.memo[id] {
            return value;
        }
        self.evaluations += 1;
        let value = match self.nodes[id] {
            Node::A => Output::Vector(input.a),
            Node::B => Output::Vector(input.b),
            Node::Q => Output::Quaternion(input.q),
            Node::T => Output::Scalar(input.t),
            Node::Constant(v) => v,
            Node::ScalarAdd(a, b) => {
                Output::Scalar(scalar(self.eval_node(a, input)) + scalar(self.eval_node(b, input)))
            }
            Node::ScalarMultiply(a, b) => {
                Output::Scalar(scalar(self.eval_node(a, input)) * scalar(self.eval_node(b, input)))
            }
            Node::Clamp(a) => Output::Scalar(scalar(self.eval_node(a, input)).clamp(-1.0, 1.0)),
            Node::VectorAdd(a, b) => Output::Vector(add(
                vector(self.eval_node(a, input)),
                vector(self.eval_node(b, input)),
            )),
            Node::VectorSubtract(a, b) => Output::Vector(sub(
                vector(self.eval_node(a, input)),
                vector(self.eval_node(b, input)),
            )),
            Node::VectorScale(v, s) => Output::Vector(scale(
                vector(self.eval_node(v, input)),
                scalar(self.eval_node(s, input)),
            )),
            Node::Rotate(q, v) => Output::Vector(rotate(
                quaternion(self.eval_node(q, input)),
                vector(self.eval_node(v, input)),
            )),
            Node::Compose(a, b) => Output::Quaternion(compose(
                quaternion(self.eval_node(a, input)),
                quaternion(self.eval_node(b, input)),
            )),
        };
        self.memo[id] = Some(value);
        value
    }
}
impl NativeBackend for Graph {
    fn evaluate(&mut self, input: Input) -> Output {
        self.memo.fill(None);
        self.evaluations = 0;
        self.eval_node(self.output, input)
    }
}
fn scalar(value: Output) -> f64 {
    let Output::Scalar(value) = value else {
        panic!("invalid scalar connection")
    };
    value
}
fn vector(value: Output) -> [f64; 3] {
    let Output::Vector(value) = value else {
        panic!("invalid vector connection")
    };
    value
}
fn quaternion(value: Output) -> [f64; 4] {
    let Output::Quaternion(value) = value else {
        panic!("invalid quaternion connection")
    };
    value
}

#[derive(Clone, Copy)]
enum Slot {
    Scalar(usize),
    Vector(usize),
    Quaternion(usize),
}
impl Slot {
    fn s(self) -> usize {
        let Self::Scalar(i) = self else {
            panic!("scalar port")
        };
        i
    }
    fn v(self) -> usize {
        let Self::Vector(i) = self else {
            panic!("vector port")
        };
        i
    }
    fn q(self) -> usize {
        let Self::Quaternion(i) = self else {
            panic!("quaternion port")
        };
        i
    }
}
enum Op {
    ScalarAdd(usize, usize, usize),
    ScalarMultiply(usize, usize, usize),
    Clamp(usize, usize),
    VectorAdd(usize, usize, usize),
    VectorSubtract(usize, usize, usize),
    VectorScale(usize, usize, usize),
    Rotate(usize, usize, usize),
    Compose(usize, usize, usize),
}

pub struct FlatPlan {
    ops: Vec<Op>,
    scalars: Vec<f64>,
    vectors: Vec<[f64; 3]>,
    quaternions: Vec<[f64; 4]>,
    output: Slot,
}
impl FlatPlan {
    pub fn new(kernel: Kernel) -> Self {
        Self::lower(&Graph::new(kernel))
    }

    pub fn lower(graph: &Graph) -> Self {
        let mut plan = Self {
            ops: Vec::new(),
            scalars: Vec::new(),
            vectors: Vec::new(),
            quaternions: Vec::new(),
            output: Slot::Scalar(0),
        };
        let mut slots: Vec<Slot> = Vec::new();
        // Benchmark graphs are constructed in dependency order. Production
        // lowering must additionally validate arbitrary authored connections.
        for node in &graph.nodes {
            let slot = match *node {
                Node::A | Node::B => plan.allocate(Output::Vector([0.0; 3])),
                Node::Q => plan.allocate(Output::Quaternion([0.0; 4])),
                Node::T => plan.allocate(Output::Scalar(0.0)),
                Node::Constant(value) => plan.allocate(value),
                Node::ScalarAdd(a, b) | Node::ScalarMultiply(a, b) => {
                    let dest = plan.allocate(Output::Scalar(0.0));
                    let a = slots[a].s();
                    let b = slots[b].s();
                    plan.ops.push(if matches!(node, Node::ScalarAdd(..)) {
                        Op::ScalarAdd(a, b, dest.s())
                    } else {
                        Op::ScalarMultiply(a, b, dest.s())
                    });
                    dest
                }
                Node::Clamp(a) => {
                    let dest = plan.allocate(Output::Scalar(0.0));
                    plan.ops.push(Op::Clamp(slots[a].s(), dest.s()));
                    dest
                }
                Node::VectorAdd(a, b) | Node::VectorSubtract(a, b) => {
                    let dest = plan.allocate(Output::Vector([0.0; 3]));
                    let a = slots[a].v();
                    let b = slots[b].v();
                    plan.ops.push(if matches!(node, Node::VectorAdd(..)) {
                        Op::VectorAdd(a, b, dest.v())
                    } else {
                        Op::VectorSubtract(a, b, dest.v())
                    });
                    dest
                }
                Node::VectorScale(v, s) => {
                    let dest = plan.allocate(Output::Vector([0.0; 3]));
                    plan.ops
                        .push(Op::VectorScale(slots[v].v(), slots[s].s(), dest.v()));
                    dest
                }
                Node::Rotate(q, v) => {
                    let dest = plan.allocate(Output::Vector([0.0; 3]));
                    plan.ops
                        .push(Op::Rotate(slots[q].q(), slots[v].v(), dest.v()));
                    dest
                }
                Node::Compose(a, b) => {
                    let dest = plan.allocate(Output::Quaternion([0.0; 4]));
                    plan.ops
                        .push(Op::Compose(slots[a].q(), slots[b].q(), dest.q()));
                    dest
                }
            };
            slots.push(slot);
        }
        plan.output = slots[graph.output];
        plan
    }

    fn allocate(&mut self, value: Output) -> Slot {
        match value {
            Output::Scalar(value) => {
                let i = self.scalars.len();
                self.scalars.push(value);
                Slot::Scalar(i)
            }
            Output::Vector(value) => {
                let i = self.vectors.len();
                self.vectors.push(value);
                Slot::Vector(i)
            }
            Output::Quaternion(value) => {
                let i = self.quaternions.len();
                self.quaternions.push(value);
                Slot::Quaternion(i)
            }
        }
    }
}
impl NativeBackend for FlatPlan {
    fn evaluate(&mut self, input: Input) -> Output {
        // Source slots were bound during lowering, not looked up by node ID here.
        self.vectors[0] = input.a;
        self.vectors[1] = input.b;
        self.quaternions[0] = input.q;
        self.scalars[0] = input.t;
        for op in &self.ops {
            match *op {
                Op::ScalarAdd(a, b, d) => self.scalars[d] = self.scalars[a] + self.scalars[b],
                Op::ScalarMultiply(a, b, d) => self.scalars[d] = self.scalars[a] * self.scalars[b],
                Op::Clamp(a, d) => self.scalars[d] = self.scalars[a].clamp(-1.0, 1.0),
                Op::VectorAdd(a, b, d) => self.vectors[d] = add(self.vectors[a], self.vectors[b]),
                Op::VectorSubtract(a, b, d) => {
                    self.vectors[d] = sub(self.vectors[a], self.vectors[b])
                }
                Op::VectorScale(v, s, d) => {
                    self.vectors[d] = scale(self.vectors[v], self.scalars[s])
                }
                Op::Rotate(q, v, d) => {
                    self.vectors[d] = rotate(self.quaternions[q], self.vectors[v])
                }
                Op::Compose(a, b, d) => {
                    self.quaternions[d] = compose(self.quaternions[a], self.quaternions[b])
                }
            }
        }
        match self.output {
            Slot::Scalar(i) => Output::Scalar(self.scalars[i]),
            Slot::Vector(i) => Output::Vector(self.vectors[i]),
            Slot::Quaternion(i) => Output::Quaternion(self.quaternions[i]),
        }
    }
}
