use std::process::Command;

fn main() {
    // compile proto file
    // 使用 protos 目录下的 protobuf 文件，输出到 src/pb 目录下
    tonic_build::configure()
        .out_dir("src/pb")
        .type_attribute("reservation.ReservationStatus", "#[derive(sqlx::Type)]")
        .compile(&["protos/reservation.proto"], &["protos"])
        .unwrap();

    // run cargo fmt the generated code
    Command::new("cargo").args(["fmt"]).output().unwrap();

    // recompile if proto file changes
    println!("cargo:rerun-if-changed=protos/reservation.proto")
}
