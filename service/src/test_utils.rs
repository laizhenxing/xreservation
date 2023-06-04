use abi::Config;
use std::path::Path;
use xsqlx_db_tester::TestDB;

/// TestConfig is a helper struct for testing. It's for unbinding the database connection[sqlx]
/// It creates a new database for each test and drops it after the test.
#[cfg(test)]
pub struct TestConfig {
    #[allow(dead_code)]
    tdb: TestDB,
    pub config: Config,
}

impl TestConfig {
    pub fn new(filename: impl AsRef<Path>) -> Self {
        let mut config = Config::load(filename).unwrap();

        let url = config.db.server_url();
        let tdb = TestDB::new(url, "../migrations");
        // 替换config.db.dbname 为新生成的dbname
        config.db.dbname = tdb.dbname.to_string();

        Self { tdb, config }
    }

    #[allow(dead_code)]
    pub fn with_server_port(port: u16) -> Self {
        let mut config = Self::default();
        config.config.server.port = port;
        config
    }
}

impl Default for TestConfig {
    fn default() -> Self {
        let filename = "fitures/config.yml";
        Self::new(filename)
    }
}
