T {
    name = "outer"
    T {
        name = "btn_a"
        Style.padding_xy(2.0, 1.0).background_color([0.30, 0.55, 1.00, 1.0]) {}
        Text { "hello" }
    }
}

let b = query("#btn_a")
print(b)
let o = query("#outer")
print(o)
