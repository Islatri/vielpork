// Deprecate


// use serde::{Serialize, Deserialize};

// // 哈希配置结构
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct HashPolicyConfig {
//     pub algorithm: HashAlgorithm,
//     pub source: HashSource,
//     pub format: HashFormat,
// }

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub enum HashAlgorithm {
//     MD5,
//     SHA1,
//     SHA256,
//     Blake3,
// }

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub enum HashSource {
//     ResourceIdentifier,  // 基于URL/ID的哈希
//     ContentPreview,      // 基于文件开头部分的哈希
//     FullContent,         // 基于完整内容的哈希（下载后）
// }

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub enum HashFormat {
//     Hex,
//     Base64,
//     Custom(String), // 例如："file_{hash:.8}.{ext}"
// }

// // 哈希计算 trait
// trait DigestHasher {
//     fn create_hasher(&self) -> Box<dyn Digest>;
//     fn output_size(&self) -> usize;
// }

// impl DigestHasher for HashAlgorithm {
//     fn create_hasher(&self) -> Box<dyn Digest> {
//         match self {
//             Self::MD5 => Box::new(md5::Context::new()),
//             Self::SHA1 => Box::new(sha1::Sha1::new()),
//             Self::SHA256 => Box::new(sha2::Sha256::new()),
//             Self::Blake3 => Box::new(blake3::Hasher::new()),
//         }
//     }

//     fn output_size(&self) -> usize {
//         match self {
//             Self::MD5 => 16,
//             Self::SHA1 => 20,
//             Self::SHA256 => 32,
//             Self::Blake3 => 32,
//         }
//     }
// }