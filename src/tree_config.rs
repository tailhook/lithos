#[deriving(Decodable)]
pub struct TreeConfig {
    config_dir: Path,
    state_dir: Path,
    readonly_paths: Vec<Path>,
    writable_paths: Vec<Path>,
    devices: Path,
}
