//! Choose a per-eye swapchain extent from a scene request and runtime limits.
pub(crate) fn select(
    requested: Option<[u32; 2]>,
    recommended: [u32; 2],
    maximum: [u32; 2],
) -> Result<[u32; 2], String> {
    let size = requested.unwrap_or(recommended);
    if size.contains(&0) || maximum.contains(&0) {
        return Err("OpenXR eye resolution and maximum must be nonzero".into());
    }
    Ok([size[0].min(maximum[0]), size[1].min(maximum[1])])
}

#[cfg(test)]
mod tests {
    use super::select;
    #[test]
    fn uses_runtime_recommendation_or_clamps_requested_size() {
        assert_eq!(
            select(None, [1712, 1712], [3440, 3440]).unwrap(),
            [1712, 1712]
        );
        assert_eq!(
            select(Some([4000, 800]), [1712, 1712], [3440, 3000]).unwrap(),
            [3440, 800]
        );
        assert!(select(Some([0, 10]), [1712, 1712], [3440, 3440]).is_err());
    }
}
