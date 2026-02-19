fn main() -> std::io::Result<()> {
    tonic_prost_build::compile_protos("src/camera_backup.proto")
}
