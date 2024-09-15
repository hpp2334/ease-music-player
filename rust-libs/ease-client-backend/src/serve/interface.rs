pub trait IServer: Send + Sync + 'static {
    fn add_image(&self, buf: Vec<u8>) -> String;
}
