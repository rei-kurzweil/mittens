//! Manual GPU regression controls are intentionally absent from the demo UI.
pub(super) fn desktop_with_detail_control() -> String {
    include_str!("../../../examples/sakura-drone-conversion.mms")
        .replace(
            "let desktop_view_enabled = false",
            "let desktop_view_enabled = true",
        )
        .replace("let xr_view_enabled = true", "let xr_view_enabled = false")
        .replace(
            "\nregenerate_buildings()\n",
            r#"
regenerate_buildings()
let detail_test_slider=Slider.range(0,8).step(1) {name="sakura_building_detail_slider"}
detail_test_slider
on(detail_test_slider,"SliderChanged",fn(event){regenerate_buildings()})
"#,
        )
}
