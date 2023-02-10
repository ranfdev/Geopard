use std::borrow::Cow;

use async_trait::async_trait;
use futures::prelude::*;

#[async_trait(?Send)]
pub trait LossyTextRead {
    async fn read_line_lossy(&mut self, buf: &mut String) -> std::io::Result<usize>;
}

#[async_trait(?Send)]
impl<T: AsyncBufRead + Unpin> LossyTextRead for T {
    async fn read_line_lossy(&mut self, buf: &mut String) -> std::io::Result<usize> {
        // FIXME:  thread 'main' panicked at 'assertion failed: self.is_char_boundary(new_len)', /build/rustc-1.58.1-src/library/alloc/src/string.rs:1204:13
        // This is safe because we treat buf as a mut Vec to read the data, BUT,
        // we check if it's valid utf8 using String::from_utf8_lossy.
        // If it's not valid utf8, we swap our buf with the newly allocated and
        // safe string returned from String::from_utf8_lossy
        //
        // In the implementation of BufReader::read_line, they talk about some things about
        // panic handling, which I don't understand currently. Whatever...
        unsafe {
            let vec_buf = buf.as_mut_vec();
            let mut n = self.read_until(b'\n', vec_buf).await?;

            let correct_string = String::from_utf8_lossy(vec_buf);
            if let Cow::Owned(valid_utf8_string) = correct_string {
                // Yes, I know this is not good for performance because it requires useless copying.
                // BUT, this code will only be executed when invalid utf8 is found, so i
                // consider this as good enough
                buf.truncate(buf.len() - n); // Remove bad non-utf8 data
                buf.push_str(&valid_utf8_string); // Add correct utf8 data instead
                n = valid_utf8_string.len();
            }
            Ok(n)
        }
    }
}
