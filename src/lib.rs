mod alc;
mod fec;

pub mod tools;

pub fn hello_world() {
    log::info!("Hello World")
}

#[cfg(test)]
mod tests {


    fn init() {
        std::env::set_var("RUST_LOG", "debug");
        env_logger::builder().is_test(true).init()
    }

    #[test]
    pub fn test_hello_world() {
        init();
        super::hello_world()
    }
}
