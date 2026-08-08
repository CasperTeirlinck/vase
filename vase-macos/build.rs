fn main() {
    // Link the private SkyLight framework for the cross-display window-focus routine in ax.rs (see `focus_window_skylight`).
    // Private frameworks aren't on the default framework search path.
    println!("cargo:rustc-link-search=framework=/System/Library/PrivateFrameworks");
    println!("cargo:rustc-link-lib=framework=SkyLight");
}
