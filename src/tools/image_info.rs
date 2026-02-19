use crate::tools::Tool;
use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

pub struct ImageInfoTool;

#[derive(Deserialize)]
struct Req { path: String }

#[async_trait]
impl Tool for ImageInfoTool {
    fn name(&self) -> &'static str { "image_info" }

    async fn run(&self, input: Value) -> Result<Value> {
        let req: Req = serde_json::from_value(input)?;
        let reader = image::ImageReader::open(&req.path)?.with_guessed_format()?;
        let format = format!("{:?}", reader.format());
        let img = reader.decode()?;
        Ok(json!({"width": img.width(), "height": img.height(), "format": format}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[tokio::test]
    async fn reads_png_info() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("t.png");
        let img = image::RgbaImage::new(10, 20);
        img.save(&p).unwrap();
        let out = ImageInfoTool.run(json!({"path":p})).await.unwrap();
        assert_eq!(out["width"], 10);
        assert_eq!(out["height"], 20);
    }
}
