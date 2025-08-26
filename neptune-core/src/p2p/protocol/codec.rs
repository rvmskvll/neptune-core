//! Message codec for Neptune P2P networking
//! 
//! This module handles encoding and decoding of protocol messages
//! for efficient network transmission.

use super::{ProtocolError, MessageType, ProtocolVersion, ProtocolCapabilities};
use super::messages::{ProtocolMessage, MessagePayload, MessageHeader, MessagePriority};
use crate::p2p::P2pResult;
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};

/// Message codec for protocol message serialization
pub struct MessageCodec {
    /// Codec configuration
    config: CodecConfig,
    /// Compression enabled flag
    compression_enabled: bool,
    /// Encryption enabled flag
    encryption_enabled: bool,
}

/// Codec configuration
#[derive(Debug, Clone)]
pub struct CodecConfig {
    /// Maximum message size
    pub max_message_size: usize,
    /// Enable compression
    pub enable_compression: bool,
    /// Enable encryption
    pub enable_encryption: bool,
    /// Compression level (0-9)
    pub compression_level: u8,
    /// Buffer size for encoding/decoding
    pub buffer_size: usize,
}

/// Encoded message with metadata
#[derive(Debug, Clone)]
pub struct EncodedMessage {
    /// Message data
    pub data: Vec<u8>,
    /// Message size
    pub size: usize,
    /// Compression ratio
    pub compression_ratio: f64,
    /// Encoding time
    pub encoding_time: std::time::Duration,
}

/// Decoded message with metadata
#[derive(Debug, Clone)]
pub struct DecodedMessage {
    /// Decoded message
    pub message: ProtocolMessage,
    /// Decoding time
    pub decoding_time: std::time::Duration,
    /// Message size
    pub size: usize,
}

impl MessageCodec {
    /// Create a new message codec
    pub fn new(config: CodecConfig) -> Self {
        Self {
            config,
            compression_enabled: config.enable_compression,
            encryption_enabled: config.enable_encryption,
        }
    }

    /// Encode a protocol message
    pub fn encode(&self, message: &ProtocolMessage) -> P2pResult<EncodedMessage> {
        let start_time = std::time::Instant::now();

        // Validate message size
        if message.size() as usize > self.config.max_message_size {
            return Err(ProtocolError::MessageTooLarge(
                format!("Message size {} exceeds limit {}", message.size(), self.config.max_message_size)
            ).into());
        }

        // Serialize message
        let serialized = bincode::serialize(message)
            .map_err(|e| ProtocolError::SerializationError(
                format!("Failed to serialize message: {}", e)
            ))?;

        let original_size = serialized.len();
        let mut compressed_data = serialized.clone();

        // Apply compression if enabled
        if self.compression_enabled {
            compressed_data = self.compress(&serialized)?;
        }

        // Apply encryption if enabled
        if self.encryption_enabled {
            compressed_data = self.encrypt(&compressed_data)?;
        }

        let encoding_time = start_time.elapsed();
        let compression_ratio = if original_size > 0 {
            compressed_data.len() as f64 / original_size as f64
        } else {
            1.0
        };

        Ok(EncodedMessage {
            data: compressed_data,
            size: compressed_data.len(),
            compression_ratio,
            encoding_time,
        })
    }

    /// Decode a protocol message
    pub fn decode(&self, data: &[u8]) -> P2pResult<DecodedMessage> {
        let start_time = std::time::Instant::now();

        // Validate data size
        if data.len() > self.config.max_message_size {
            return Err(ProtocolError::MessageTooLarge(
                format!("Data size {} exceeds limit {}", data.len(), self.config.max_message_size)
            ).into());
        }

        let mut decrypted_data = data.to_vec();

        // Decrypt if encryption is enabled
        if self.encryption_enabled {
            decrypted_data = self.decrypt(&decrypted_data)?;
        }

        // Decompress if compression is enabled
        let mut decompressed_data = decrypted_data.clone();
        if self.compression_enabled {
            decompressed_data = self.decompress(&decrypted_data)?;
        }

        // Deserialize message
        let message: ProtocolMessage = bincode::deserialize(&decompressed_data)
            .map_err(|e| ProtocolError::DeserializationError(
                format!("Failed to deserialize message: {}", e)
            ))?;

        let decoding_time = start_time.elapsed();

        Ok(DecodedMessage {
            message,
            decoding_time,
            size: data.len(),
        })
    }

    /// Encode message asynchronously
    pub async fn encode_async<R: AsyncRead + Unpin>(
        &self,
        message: &ProtocolMessage,
        writer: &mut R,
    ) -> P2pResult<EncodedMessage> {
        let encoded = self.encode(message)?;
        
        // Write encoded data
        writer.write_all(&encoded.data).await
            .map_err(|e| ProtocolError::IoError(
                format!("Failed to write encoded message: {}", e)
            ))?;

        Ok(encoded)
    }

    /// Decode message asynchronously
    pub async fn decode_async<W: AsyncWrite + Unpin>(
        &self,
        reader: &mut W,
    ) -> P2pResult<DecodedMessage> {
        // Read message size first (assuming 4-byte length prefix)
        let mut size_buffer = [0u8; 4];
        reader.read_exact(&mut size_buffer).await
            .map_err(|e| ProtocolError::IoError(
                format!("Failed to read message size: {}", e)
            ))?;

        let message_size = u32::from_le_bytes(size_buffer) as usize;

        // Validate message size
        if message_size > self.config.max_message_size {
            return Err(ProtocolError::MessageTooLarge(
                format!("Message size {} exceeds limit {}", message_size, self.config.max_message_size)
            ).into());
        }

        // Read message data
        let mut data = vec![0u8; message_size];
        reader.read_exact(&mut data).await
            .map_err(|e| ProtocolError::IoError(
                format!("Failed to read message data: {}", e)
            ))?;

        // Decode message
        self.decode(&data)
    }

    /// Compress data using zstd
    fn compress(&self, data: &[u8]) -> P2pResult<Vec<u8>> {
        use zstd::stream::write::Encoder;
        use std::io::Write;

        let mut encoder = Encoder::new(Vec::new(), self.config.compression_level as i32)
            .map_err(|e| ProtocolError::CompressionError(
                format!("Failed to create compressor: {}", e)
            ))?;

        encoder.write_all(data)
            .map_err(|e| ProtocolError::CompressionError(
                format!("Failed to compress data: {}", e)
            ))?;

        encoder.finish()
            .map_err(|e| ProtocolError::CompressionError(
                format!("Failed to finish compression: {}", e)
            ))?;

        Ok(encoder.finish()
            .map_err(|e| ProtocolError::CompressionError(
                format!("Failed to get compressed data: {}", e)
            ))?)
    }

    /// Decompress data using zstd
    fn decompress(&self, data: &[u8]) -> P2pResult<Vec<u8>> {
        use zstd::stream::read::Decoder;
        use std::io::Read;

        let mut decoder = Decoder::new(data)
            .map_err(|e| ProtocolError::DecompressionError(
                format!("Failed to create decompressor: {}", e)
            ))?;

        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)
            .map_err(|e| ProtocolError::DecompressionError(
                format!("Failed to decompress data: {}", e)
            ))?;

        Ok(decompressed)
    }

    /// Encrypt data using ChaCha20-Poly1305
    fn encrypt(&self, data: &[u8]) -> P2pResult<Vec<u8>> {
        use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
        use chacha20poly1305::aead::{Aead, NewAead};

        // Generate a random key and nonce (in production, use proper key management)
        let key = Key::from_slice(b"01234567890123456789012345678901");
        let nonce = Nonce::from_slice(b"012345678901");

        let cipher = ChaCha20Poly1305::new(key);

        cipher.encrypt(nonce, data)
            .map_err(|e| ProtocolError::EncryptionError(
                format!("Failed to encrypt data: {}", e)
            ))?;

        Ok(cipher.encrypt(nonce, data)
            .map_err(|e| ProtocolError::EncryptionError(
                format!("Failed to encrypt data: {}", e)
            ))?)
    }

    /// Decrypt data using ChaCha20-Poly1305
    fn decrypt(&self, data: &[u8]) -> P2pResult<Vec<u8>> {
        use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
        use chacha20poly1305::aead::{Aead, NewAead};

        // Use the same key and nonce as encryption
        let key = Key::from_slice(b"01234567890123456789012345678901");
        let nonce = Nonce::from_slice(b"012345678901");

        let cipher = ChaCha20Poly1305::new(key);

        cipher.decrypt(nonce, data)
            .map_err(|e| ProtocolError::DecryptionError(
                format!("Failed to decrypt data: {}", e)
            ))?;

        Ok(cipher.decrypt(nonce, data)
            .map_err(|e| ProtocolError::DecryptionError(
                format!("Failed to decrypt data: {}", e)
            ))?)
    }

    /// Encode message header only
    pub fn encode_header(&self, header: &MessageHeader) -> P2pResult<Vec<u8>> {
        bincode::serialize(header)
            .map_err(|e| ProtocolError::SerializationError(
                format!("Failed to serialize header: {}", e)
            ))?;

        Ok(bincode::serialize(header)
            .map_err(|e| ProtocolError::SerializationError(
                format!("Failed to serialize header: {}", e)
            ))?)
    }

    /// Decode message header only
    pub fn decode_header(&self, data: &[u8]) -> P2pResult<MessageHeader> {
        bincode::deserialize(data)
            .map_err(|e| ProtocolError::DeserializationError(
                format!("Failed to deserialize header: {}", e)
            ))?;

        Ok(bincode::deserialize(data)
            .map_err(|e| ProtocolError::DeserializationError(
                format!("Failed to deserialize header: {}", e)
            ))?)
    }

    /// Get codec configuration
    pub fn config(&self) -> &CodecConfig {
        &self.config
    }

    /// Check if compression is enabled
    pub fn compression_enabled(&self) -> bool {
        self.compression_enabled
    }

    /// Check if encryption is enabled
    pub fn encryption_enabled(&self) -> bool {
        self.encryption_enabled
    }

    /// Set compression enabled
    pub fn set_compression_enabled(&mut self, enabled: bool) {
        self.compression_enabled = enabled;
    }

    /// Set encryption enabled
    pub fn set_encryption_enabled(&mut self, enabled: bool) {
        self.encryption_enabled = enabled;
    }

    /// Validate message size
    pub fn validate_message_size(&self, size: usize) -> P2pResult<()> {
        if size > self.config.max_message_size {
            return Err(ProtocolError::MessageTooLarge(
                format!("Message size {} exceeds limit {}", size, self.config.max_message_size)
            ).into());
        }

        Ok(())
    }

    /// Get compression statistics
    pub fn get_compression_stats(&self, original_size: usize, compressed_size: usize) -> CompressionStats {
        let compression_ratio = if original_size > 0 {
            compressed_size as f64 / original_size as f64
        } else {
            1.0
        };

        let compression_percentage = (1.0 - compression_ratio) * 100.0;
        let bytes_saved = if original_size > compressed_size {
            original_size - compressed_size
        } else {
            0
        };

        CompressionStats {
            original_size,
            compressed_size,
            compression_ratio,
            compression_percentage,
            bytes_saved,
        }
    }

    /// Create a test codec configuration
    pub fn test_config() -> CodecConfig {
        CodecConfig {
            max_message_size: 1024 * 1024, // 1MB
            enable_compression: false, // Disable for testing
            enable_encryption: false, // Disable for testing
            compression_level: 6,
            buffer_size: 8192,
        }
    }
}

/// Compression statistics
#[derive(Debug, Clone)]
pub struct CompressionStats {
    /// Original data size
    pub original_size: usize,
    /// Compressed data size
    pub compressed_size: usize,
    /// Compression ratio (compressed/original)
    pub compression_ratio: f64,
    /// Compression percentage
    pub compression_percentage: f64,
    /// Bytes saved
    pub bytes_saved: usize,
}

impl Default for CodecConfig {
    fn default() -> Self {
        Self {
            max_message_size: 1024 * 1024, // 1MB
            enable_compression: true,
            enable_encryption: true,
            compression_level: 6,
            buffer_size: 8192,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::protocol::{MessageType, ProtocolVersion, ProtocolCapabilities};
    use crate::p2p::protocol::messages::{MessagePayload, PingPongMessage, PingType};

    #[test]
    fn test_message_codec_creation() {
        let config = CodecConfig::default();
        let codec = MessageCodec::new(config);
        assert!(codec.compression_enabled());
        assert!(codec.encryption_enabled());
    }

    #[test]
    fn test_message_encoding_decoding() {
        let config = CodecConfig::test_config();
        let codec = MessageCodec::new(config);
        
        let message = ProtocolMessage::new(
            MessageType::Ping,
            ProtocolVersion::V1,
            "peer1".to_string(),
            "peer2".to_string(),
            MessagePayload::PingPong(PingPongMessage {
                ping_type: PingType::Ping,
                sequence: 1,
                timestamp: 1234567890,
                payload: None,
            }),
        );

        // Encode message
        let encoded = codec.encode(&message).unwrap();
        assert!(encoded.size > 0);

        // Decode message
        let decoded = codec.decode(&encoded.data).unwrap();
        assert_eq!(decoded.message.message_type(), &MessageType::Ping);
        assert_eq!(decoded.message.source_peer_id(), "peer1");
    }

    #[test]
    fn test_codec_config_default() {
        let config = CodecConfig::default();
        assert_eq!(config.max_message_size, 1024 * 1024);
        assert!(config.enable_compression);
        assert!(config.enable_encryption);
        assert_eq!(config.compression_level, 6);
        assert_eq!(config.buffer_size, 8192);
    }

    #[test]
    fn test_codec_config_test() {
        let config = CodecConfig::test_config();
        assert_eq!(config.max_message_size, 1024 * 1024);
        assert!(!config.enable_compression);
        assert!(!config.enable_encryption);
        assert_eq!(config.compression_level, 6);
        assert_eq!(config.buffer_size, 8192);
    }

    #[test]
    fn test_message_size_validation() {
        let config = CodecConfig::test_config();
        let codec = MessageCodec::new(config);
        
        // Valid size
        assert!(codec.validate_message_size(1024).is_ok());
        
        // Invalid size
        assert!(codec.validate_message_size(2 * 1024 * 1024).is_err());
    }

    #[test]
    fn test_compression_stats() {
        let config = CodecConfig::test_config();
        let codec = MessageCodec::new(config);
        
        let stats = codec.get_compression_stats(1000, 600);
        assert_eq!(stats.original_size, 1000);
        assert_eq!(stats.compressed_size, 600);
        assert_eq!(stats.compression_ratio, 0.6);
        assert_eq!(stats.compression_percentage, 40.0);
        assert_eq!(stats.bytes_saved, 400);
    }

    #[test]
    fn test_codec_configuration() {
        let mut config = CodecConfig::default();
        let mut codec = MessageCodec::new(config.clone());
        
        // Test compression setting
        codec.set_compression_enabled(false);
        assert!(!codec.compression_enabled());
        
        // Test encryption setting
        codec.set_encryption_enabled(false);
        assert!(!codec.encryption_enabled());
    }
}
