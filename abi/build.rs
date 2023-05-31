use std::process::Command;

use tonic_build::Builder;

fn main() {
    // compile proto file
    // 使用 protos 目录下的 protobuf 文件，输出到 src/pb 目录下
    tonic_build::configure()
        .out_dir("src/pb")
        .with_sql_type(&["reservation.ReservationStatus"])
        .with_builder(&[
            "reservation.ReservationQuery",
            "reservation.ReservationFilter",
        ])
        .with_builder_into(
            "reservation.ReservationQuery",
            &[
                "resource_id",
                "user_id",
                "status",
                "page",
                "page_size",
                "desc",
            ],
        )
        .with_builder_into(
            "reservation.ReservationFilter",
            &[
                "resource_id",
                "user_id",
                "status",
                "cursor",
                "page_size",
                "desc",
            ],
        )
        .with_builder_option("reservation.ReservationQuery", &["start", "end"])
        .compile(&["protos/reservation.proto"], &["protos"])
        .unwrap();

    // run cargo fmt the generated code
    Command::new("cargo").args(["fmt"]).output().unwrap();

    // recompile if proto file changes
    println!("cargo:rerun-if-changed=protos/reservation.proto")
}

/// 为 tonic_build::Builder 添加扩展方法,用于设置属性
trait BuilderExt {
    fn with_sql_type(self, paths: &[&str]) -> Self;
    fn with_builder(self, paths: &[&str]) -> Self;
    fn with_builder_into(self, path: &str, fields: &[&str]) -> Self;
    fn with_builder_option(self, path: &str, fields: &[&str]) -> Self;
}

impl BuilderExt for Builder {
    // fold 用法: 每次迭代都会将上一次的结果作为参数传入,最后返回最后一次的结果
    // 在这里指的是 Builder [acc 就是 self]
    fn with_sql_type(self, paths: &[&str]) -> Self {
        paths.iter().fold(self, |acc, path| {
            acc.type_attribute(path, "#[derive(sqlx::Type)]")
        })
    }

    fn with_builder(self, paths: &[&str]) -> Self {
        paths.iter().fold(self, |acc, path| {
            acc.type_attribute(path, "#[derive(derive_builder::Builder)]")
        })
    }

    fn with_builder_into(self, path: &str, fields: &[&str]) -> Self {
        fields.iter().fold(self, |acc, field| {
            let field = format!("{}.{}", path, field);
            // 指明 default 之后,可以在初始化时不对该字段设置值
            acc.field_attribute(field, "#[builder(setter(into), default)]")
        })
    }

    fn with_builder_option(self, path: &str, fields: &[&str]) -> Self {
        fields.iter().fold(self, |acc, field| {
            let field = format!("{}.{}", path, field);
            acc.field_attribute(field, "#[builder(setter(strip_option))]")
        })
    }
}
