use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct CompactSize {
    pub value: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BitcoinError {
    InsufficientBytes,
    InvalidFormat,
}

impl CompactSize {
    pub fn new(value: u64) -> Self {
        CompactSize { value }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self.value {
            0..=0xFC => vec![self.value as u8],
            0xFD..=0xFFFF => {
                let mut bytes = vec![0xFD];
                bytes.extend_from_slice(&(self.value as u16).to_le_bytes());
                bytes
            }
            0x10000..=0xFFFFFFFF => {
                let mut bytes = vec![0xFE];
                bytes.extend_from_slice(&(self.value as u32).to_le_bytes());
                bytes
            }
            _ => {
                let mut bytes = vec![0xFF];
                bytes.extend_from_slice(&self.value.to_le_bytes());
                bytes
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.is_empty() {
            return Err(BitcoinError::InsufficientBytes);
        }

        match bytes[0] {
            0x00..=0xFC => Ok((CompactSize::new(bytes[0] as u64), 1)),
            0xFD => {
                if bytes.len() < 3 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let value = u16::from_le_bytes([bytes[1], bytes[2]]) as u64;
                Ok((CompactSize::new(value), 3))
            }
            0xFE => {
                if bytes.len() < 5 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let value = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as u64;
                Ok((CompactSize::new(value), 5))
            }
            0xFF => {
                if bytes.len() < 9 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let value = u64::from_le_bytes([
                    bytes[1], bytes[2], bytes[3], bytes[4],
                    bytes[5], bytes[6], bytes[7], bytes[8],
                ]);
                Ok((CompactSize::new(value), 9))
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Txid(pub [u8; 32]);

impl Serialize for Txid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let hex_string = hex::encode(&self.0);
        serializer.serialize_str(&hex_string)
    }
}

impl<'de> Deserialize<'de> for Txid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let hex_string = String::deserialize(deserializer)?;
        let bytes = hex::decode(&hex_string).map_err(serde::de::Error::custom)?;
        if bytes.len() != 32 {
            return Err(serde::de::Error::custom("Txid must be exactly 32 bytes"));
        }
        let mut array = [0u8; 32];
        array.copy_from_slice(&bytes);
        Ok(Txid(array))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct OutPoint {
    pub txid: Txid,
    pub vout: u32,
}

impl OutPoint {
    pub fn new(txid: [u8; 32], vout: u32) -> Self {
        OutPoint {
            txid: Txid(txid),
            vout,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.txid.0);
        bytes.extend_from_slice(&self.vout.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 36 {
            return Err(BitcoinError::InsufficientBytes);
        }
        
        let mut txid = [0u8; 32];
        txid.copy_from_slice(&bytes[0..32]);
        let vout = u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]);
        
        Ok((OutPoint::new(txid, vout), 36))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Script {
    pub bytes: Vec<u8>,
}

impl Script {
    pub fn new(bytes: Vec<u8>) -> Self {
        Script { bytes }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();
        let length = CompactSize::new(self.bytes.len() as u64);
        result.extend_from_slice(&length.to_bytes());
        result.extend_from_slice(&self.bytes);
        result
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (length, consumed) = CompactSize::from_bytes(bytes)?;
        let total_bytes_needed = consumed + length.value as usize;
        
        if bytes.len() < total_bytes_needed {
            return Err(BitcoinError::InsufficientBytes);
        }
        
        let script_bytes = bytes[consumed..total_bytes_needed].to_vec();
        Ok((Script::new(script_bytes), total_bytes_needed))
    }
}

impl Deref for Script {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub previous_output: OutPoint,
    pub script_sig: Script,
    pub sequence: u32,
}

impl TransactionInput {
    pub fn new(previous_output: OutPoint, script_sig: Script, sequence: u32) -> Self {
        TransactionInput {
            previous_output,
            script_sig,
            sequence,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.previous_output.to_bytes());
        bytes.extend_from_slice(&self.script_sig.to_bytes());
        bytes.extend_from_slice(&self.sequence.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let mut offset = 0;
        
        // Parse OutPoint (36 bytes)
        let (previous_output, consumed) = OutPoint::from_bytes(&bytes[offset..])?;
        offset += consumed;
        
        // Parse Script (with CompactSize)
        let (script_sig, consumed) = Script::from_bytes(&bytes[offset..])?;
        offset += consumed;
        
        // Parse sequence (4 bytes)
        if bytes.len() < offset + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let sequence = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);
        offset += 4;
        
        Ok((TransactionInput::new(previous_output, script_sig, sequence), offset))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BitcoinTransaction {
    pub version: u32,
    pub inputs: Vec<TransactionInput>,
    pub lock_time: u32,
}

impl BitcoinTransaction {
    pub fn new(version: u32, inputs: Vec<TransactionInput>, lock_time: u32) -> Self {
        BitcoinTransaction {
            version,
            inputs,
            lock_time,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        
        // Version (4 bytes LE)
        bytes.extend_from_slice(&self.version.to_le_bytes());
        
        // CompactSize (number of inputs)
        let input_count = CompactSize::new(self.inputs.len() as u64);
        bytes.extend_from_slice(&input_count.to_bytes());
        
        // Each input serialized
        for input in &self.inputs {
            bytes.extend_from_slice(&input.to_bytes());
        }
        
        // Lock time (4 bytes LE)
        bytes.extend_from_slice(&self.lock_time.to_le_bytes());
        
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let mut offset = 0;
        
        // Read version (4 bytes LE)
        if bytes.len() < 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let version = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        offset += 4;
        
        // Read CompactSize for input count
        let (input_count, consumed) = CompactSize::from_bytes(&bytes[offset..])?;
        offset += consumed;
        
        // Parse inputs one by one
        let mut inputs = Vec::new();
        for _ in 0..input_count.value {
            let (input, consumed) = TransactionInput::from_bytes(&bytes[offset..])?;
            inputs.push(input);
            offset += consumed;
        }
        
        // Read final 4 bytes for lock_time
        if bytes.len() < offset + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let lock_time = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);
        offset += 4;
        
        Ok((BitcoinTransaction::new(version, inputs, lock_time), offset))
    }
}

impl fmt::Display for BitcoinTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Bitcoin Transaction:")?;
        writeln!(f, "  Version: {}", self.version)?;
        writeln!(f, "  Inputs ({}): ", self.inputs.len())?;
        
        for (i, input) in self.inputs.iter().enumerate() {
            writeln!(f, "    Input {}:", i)?;
            writeln!(f, "      Previous Output Txid: {}", hex::encode(&input.previous_output.txid.0))?;
            writeln!(f, "      Previous Output Vout: {}", input.previous_output.vout)?;
            writeln!(f, "      Script Sig Length: {}", input.script_sig.bytes.len())?;
            writeln!(f, "      Script Sig: {}", hex::encode(&input.script_sig.bytes))?;
            writeln!(f, "      Sequence: 0x{:08X}", input.sequence)?;
        }
        
        write!(f, "  Lock Time: {}", self.lock_time)
    }
}
