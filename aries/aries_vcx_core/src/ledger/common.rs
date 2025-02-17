use std::collections::HashMap;

use serde::Deserialize;

use crate::errors::error::{AriesVcxCoreError, AriesVcxCoreErrorKind, VcxCoreResult};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Request {
    #[allow(dead_code)]
    pub req_id: u64,
    pub identifier: String,
    pub signature: Option<String>,
    pub signatures: Option<HashMap<String, String>>,
    pub endorser: Option<String>,
}

pub fn verify_transaction_can_be_endorsed(
    transaction_json: &str,
    submitter_did: &str,
) -> VcxCoreResult<()> {
    let transaction: Request = serde_json::from_str(transaction_json).map_err(|err| {
        AriesVcxCoreError::from_msg(AriesVcxCoreErrorKind::InvalidJson, format!("{err:?}"))
    })?;

    let endorser_did = transaction.endorser.ok_or(AriesVcxCoreError::from_msg(
        AriesVcxCoreErrorKind::InvalidJson,
        "Transaction cannot be endorsed: endorser DID is not set.",
    ))?;

    if endorser_did != submitter_did {
        return Err(AriesVcxCoreError::from_msg(
            AriesVcxCoreErrorKind::InvalidJson,
            format!(
                "Transaction cannot be endorsed: transaction endorser DID `{endorser_did}` and \
                 sender DID `{submitter_did}` are different"
            ),
        ));
    }

    let identifier = transaction.identifier.as_str();
    if transaction.signature.is_none()
        && !transaction
            .signatures
            .as_ref()
            .map(|signatures| signatures.contains_key(identifier))
            .unwrap_or(false)
    {
        return Err(AriesVcxCoreError::from_msg(
            AriesVcxCoreErrorKind::InvalidJson,
            "Transaction cannot be endorsed: the author must sign the transaction.".to_string(),
        ));
    }

    Ok(())
}
