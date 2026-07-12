#[test]
fn runtime_recommendation_surface_is_not_exported() {
    let lib_rs = include_str!("../src/lib.rs");
    let routes_rs = include_str!("../src/routes.rs");

    assert!(!lib_rs.contains("pub mod ads;"));
    assert!(!routes_rs.contains("\"/ads\""));
    assert!(!routes_rs.contains("async fn ads("));
}
