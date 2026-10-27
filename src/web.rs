include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub const MODE: &str = "device";

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub const MODE: &str = "simulation";
