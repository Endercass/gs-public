pub trait Rewriter {
    fn rewrite(&self, input: Vec<u8>) -> crate::Result<Vec<u8>>;
}
