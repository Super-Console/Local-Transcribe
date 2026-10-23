//! 中文标点后处理（可选）。
//!
//! Paraformer 一类的 ASR 模型输出不带标点；启用本模块后，每段识别文本会再过一次
//! sherpa-onnx 的 CT-Transformer 标点模型。模型文件由界面按需下载，未下载时
//! 整条转写路径与本模块无关。

use std::path::Path;

use sherpa_onnx::{OfflinePunctuation, OfflinePunctuationConfig};

pub(crate) struct Punctuator {
    inner: OfflinePunctuation,
}

impl Punctuator {
    pub(crate) fn create(model: &Path) -> Result<Self, String> {
        if !model.is_file() {
            return Err(format!(
                "标点模型文件不存在：{}。请在模型页重新下载。",
                model.display()
            ));
        }
        let mut config = OfflinePunctuationConfig::default();
        config.model.ct_transformer = Some(model.to_string_lossy().into_owned());
        config.model.num_threads = 1;
        let inner = OfflinePunctuation::create(&config)
            .ok_or_else(|| "标点模型加载失败，请在模型页重新下载。".to_owned())?;
        Ok(Self { inner })
    }

    /// 给一段识别文本补标点。模型失败时原样返回，绝不丢字。
    pub(crate) fn punctuate(&self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }
        self.inner
            .add_punctuation(text)
            .unwrap_or_else(|| text.to_owned())
    }
}
