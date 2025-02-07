use std::io::Cursor;

use actix_multipart::Multipart;
use image::ImageError;
use thiserror::Error;

use futures::{AsyncWriteExt, TryStreamExt};

pub const IMAGE_EXTS: &'static [&'static str] =
    &["PNG", "JPG", "JPEG", "GIF", "WEBP", "JFIF", "PJPEG", "PJP"];

#[derive(Error, Debug)]
pub enum MultipartError {
    #[error("field `{0}` was not found")]
    FieldNotFound(String),
    #[error("payload was larger than `{0}`")]
    PayloadTooLarge(usize),
    #[error("there was a problem writing from the payload: `{0}`")]
    WriteError(std::io::Error),
}

pub struct File {
    pub filename: String,
    pub bytes: Vec<u8>,
    pub size: usize,
}

pub fn get_thumbnail_image(bytes: &[u8]) -> Result<Vec<u8>, ImageError> {
    let mut buf = Vec::new();

    image::load_from_memory(&bytes)?
        .thumbnail(500, 500)
        .write_to(&mut Cursor::new(&mut buf), image::ImageOutputFormat::Png)?;

    Ok(buf)
}

pub async fn get_file_from_payload(
    payload: &mut Multipart,
    size_limit: usize,
    field_name: &str,
) -> Result<File, MultipartError> {
    while let Ok(Some(mut field)) = payload.try_next().await {
        let disposition = field.content_disposition().clone();
        let filename_param = match disposition.get_filename() {
            Some(v) => v,
            None => continue,
        };

        let name_param = match disposition.get_name() {
            Some(v) => v,
            None => continue,
        };

        if name_param != field_name {
            continue;
        }

        let mut bytes = Vec::<u8>::new();
        let mut size = 0;

        while let Ok(Some(chunk)) = field.try_next().await {
            size += chunk.len();

            if size > size_limit {
                return Err(MultipartError::PayloadTooLarge(size_limit));
            }

            if let Err(err) = bytes.write(&chunk).await {
                return Err(MultipartError::WriteError(err));
            }
        }

        return Ok(File {
            filename: filename_param.to_string(),
            bytes: bytes,
            size: size,
        });
    }

    Err(MultipartError::FieldNotFound(field_name.to_string()))
}
