use crate::encode_options::EncodeOptions;
use crate::types::DecodedImage;
pub fn encode(_img: &DecodedImage, _opts: &EncodeOptions) -> Option<Vec<u8>> {
    None
}
