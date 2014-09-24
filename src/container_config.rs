use std::collections::TreeMap;

/*
TODO(tailhook) use the following volume
enum Volume {
    Readonly(Path),
    Persistent(Path),
    Tmpfs(String),
}
*/

type Volume = String;

struct ContainerConfig {
    volumes: TreeMap<String, Volume>,
    memory_limit: uint,
    cpu_shares: uint,
    instances: uint,
    executable: Path,
    hostname: String,
    command: Vec<String>,
    environ: TreeMap<String, String>,
}
