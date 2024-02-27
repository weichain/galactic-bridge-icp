use k256::ecdsa::{RecoveryId, VerifyingKey};
use k256::elliptic_curve::sec1::ToEncodedPoint;

/// An ECDSA public key
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey {
    key: VerifyingKey,
}

impl PublicKey {
    /// Serialize a public key in SEC1 format
    ///
    /// The point can optionally be compressed
    ///
    /// See SEC1 <https://www.secg.org/sec1-v2.pdf> section 2.3.3 for details
    pub fn serialize_sec1(&self, compressed: bool) -> Vec<u8> {
        self.key.to_encoded_point(compressed).as_bytes().to_vec()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryId {
    recid: k256_RecoveryId,
}

impl RecoveryId {
    /// True iff the affine y-coordinate of `𝑘×𝑮` odd.
    pub const fn is_y_odd(&self) -> bool {
        self.recid.is_y_odd()
    }

    pub const fn is_x_reduced(&self) -> bool {
        self.recid.is_x_reduced()
    }

    /// Convert this [`RecoveryId`] into a `u8`.
    pub const fn to_byte(&self) -> u8 {
        self.recid.to_byte()
    }
}
