use std::collections::HashMap;

use aries_vcx::{
    handlers::{
        proof_presentation::prover::Prover,
        util::{matches_opt_thread_id, matches_thread_id},
    },
    messages::{
        msg_fields::protocols::{
            notification::Notification,
            present_proof::{
                v1::{request::RequestPresentationV1, PresentProofV1},
                PresentProof,
            },
        },
        AriesMessage,
    },
    protocols::proof_presentation::prover::state_machine::ProverState,
};
use serde_json;

use super::mediated_connection::send_message;
use crate::{
    api_vcx::{
        api_global::profile::{get_main_anoncreds, get_main_ledger_read, get_main_wallet},
        api_handle::{mediated_connection, object_cache::ObjectCache, ToU32},
    },
    errors::error::{LibvcxError, LibvcxErrorKind, LibvcxResult},
};

lazy_static! {
    static ref HANDLE_MAP: ObjectCache<Prover> =
        ObjectCache::<Prover>::new("disclosed-proofs-cache");
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "version", content = "data")]
enum DisclosedProofs {
    #[serde(rename = "2.0")]
    V3(Prover),
}

pub fn create_with_proof_request(source_id: &str, proof_req: &str) -> LibvcxResult<u32> {
    trace!(
        "create_with_proof_request >>> source_id: {}, proof_req: {}",
        source_id,
        proof_req
    );

    let presentation_request: RequestPresentationV1 =
        serde_json::from_str(proof_req).map_err(|err| {
            LibvcxError::from_msg(
                LibvcxErrorKind::InvalidJson,
                format!(
                    "Strict `aries` protocol is enabled. Can not parse `aries` formatted \
                     Presentation Request: {}\nError: {}",
                    proof_req, err
                ),
            )
        })?;

    let proof = Prover::create_from_request(source_id, presentation_request)?;
    HANDLE_MAP.add(proof)
}

pub async fn create_with_msgid(
    source_id: &str,
    connection_handle: u32,
    msg_id: &str,
) -> LibvcxResult<(u32, String)> {
    let proof_request = get_proof_request(connection_handle, msg_id).await?;

    let presentation_request: RequestPresentationV1 = serde_json::from_str(&proof_request)
        .map_err(|err| {
            LibvcxError::from_msg(
                LibvcxErrorKind::InvalidJson,
                format!(
                    "Strict `aries` protocol is enabled. Can not parse `aries` formatted \
                     Presentation Request: {}\nError: {}",
                    proof_request, err
                ),
            )
        })?;

    let proof = Prover::create_from_request(source_id, presentation_request)?;

    let handle = HANDLE_MAP.add(proof)?;

    debug!("inserting disclosed proof {} into handle map", source_id);
    Ok((handle, proof_request))
}

pub fn get_state(handle: u32) -> LibvcxResult<u32> {
    HANDLE_MAP
        .get(handle, |proof| Ok(proof.get_state().to_u32()))
        .map_err(|e| {
            LibvcxError::from_msg(LibvcxErrorKind::InvalidDisclosedProofHandle, e.to_string())
        })
}

pub fn prover_find_message_to_handle(
    sm: &Prover,
    messages: HashMap<String, AriesMessage>,
) -> Option<(String, AriesMessage)> {
    trace!("prover_find_message_to_handle >>> messages: {:?}", messages);
    for (uid, message) in messages {
        match sm.get_state() {
            ProverState::PresentationProposalSent => match &message {
                AriesMessage::ReportProblem(msg) => {
                    if matches_opt_thread_id!(msg, sm.get_thread_id().unwrap().as_str()) {
                        return Some((uid, message));
                    }
                }
                AriesMessage::Notification(Notification::ProblemReport(msg)) => {
                    if matches_opt_thread_id!(msg, sm.get_thread_id().unwrap().as_str()) {
                        return Some((uid, message));
                    }
                }
                AriesMessage::PresentProof(PresentProof::V1(
                    PresentProofV1::RequestPresentation(msg),
                )) => {
                    if matches_opt_thread_id!(msg, sm.get_thread_id().unwrap().as_str()) {
                        return Some((uid, message));
                    }
                }
                _ => {}
            },
            ProverState::PresentationSent => match &message {
                AriesMessage::Notification(Notification::Ack(msg)) => {
                    if matches_thread_id!(msg, sm.get_thread_id().unwrap().as_str()) {
                        return Some((uid, message));
                    }
                }
                AriesMessage::PresentProof(PresentProof::V1(PresentProofV1::Ack(msg))) => {
                    if matches_thread_id!(msg, sm.get_thread_id().unwrap().as_str()) {
                        return Some((uid, message));
                    }
                }
                AriesMessage::ReportProblem(msg) => {
                    if matches_opt_thread_id!(msg, sm.get_thread_id().unwrap().as_str()) {
                        return Some((uid, message));
                    }
                }
                AriesMessage::Notification(Notification::ProblemReport(msg)) => {
                    if matches_opt_thread_id!(msg, sm.get_thread_id().unwrap().as_str()) {
                        return Some((uid, message));
                    }
                }
                AriesMessage::PresentProof(PresentProof::V1(PresentProofV1::ProblemReport(
                    msg,
                ))) => {
                    if matches_opt_thread_id!(msg, sm.get_thread_id().unwrap().as_str()) {
                        return Some((uid, message));
                    }
                }
                _ => {}
            },
            _ => {}
        };
    }
    None
}

pub async fn update_state(
    handle: u32,
    message: Option<&str>,
    connection_handle: u32,
) -> LibvcxResult<u32> {
    let mut proof = HANDLE_MAP.get_cloned(handle)?;
    trace!(
        "disclosed_proof::update_state >>> connection_handle: {:?}, message: {:?}",
        connection_handle,
        message
    );
    if !proof.progressable_by_message() {
        trace!("disclosed_proof::update_state >> found no available transition");
        return Ok(proof.get_state().to_u32());
    }

    if let Some(message) = message {
        let message: AriesMessage = serde_json::from_str(message).map_err(|err| {
            LibvcxError::from_msg(
                LibvcxErrorKind::InvalidOption,
                format!(
                    "Can not updated state with message: Message deserialization failed: {:?}",
                    err
                ),
            )
        })?;
        trace!(
            "disclosed_proof::update_state >>> updating using message {:?}",
            message
        );
        proof.process_aries_msg(message).await?;
    } else {
        let messages = mediated_connection::get_messages(connection_handle).await?;
        trace!(
            "disclosed_proof::update_state >>> found messages: {:?}",
            messages
        );
        if let Some((uid, message)) = prover_find_message_to_handle(&proof, messages) {
            proof.process_aries_msg(message).await?;
            mediated_connection::update_message_status(connection_handle, &uid).await?;
        };
    }
    let state: u32 = proof.get_state().to_u32();
    HANDLE_MAP.insert(handle, proof)?;
    Ok(state)
}

pub fn to_string(handle: u32) -> LibvcxResult<String> {
    HANDLE_MAP.get(handle, |proof| {
        serde_json::to_string(&DisclosedProofs::V3(proof.clone())).map_err(|err| {
            LibvcxError::from_msg(
                LibvcxErrorKind::InvalidState,
                format!("cannot serialize DisclosedProof proofect: {:?}", err),
            )
        })
    })
}

pub fn from_string(proof_data: &str) -> LibvcxResult<u32> {
    let proof: DisclosedProofs = serde_json::from_str(proof_data).map_err(|err| {
        LibvcxError::from_msg(
            LibvcxErrorKind::InvalidJson,
            format!("cannot deserialize DisclosedProofs object: {:?}", err),
        )
    })?;

    match proof {
        DisclosedProofs::V3(proof) => HANDLE_MAP.add(proof),
    }
}

pub fn release(handle: u32) -> LibvcxResult<()> {
    HANDLE_MAP.release(handle).map_err(|e| {
        LibvcxError::from_msg(LibvcxErrorKind::InvalidDisclosedProofHandle, e.to_string())
    })
}

pub fn release_all() {
    HANDLE_MAP.drain().ok();
}

pub fn get_presentation_msg(handle: u32) -> LibvcxResult<String> {
    HANDLE_MAP.get(handle, |proof| {
        let presentation = proof.get_presentation_msg()?;
        Ok(json!(presentation).to_string())
    })
}

pub async fn send_proof(handle: u32, connection_handle: u32) -> LibvcxResult<()> {
    let mut proof = HANDLE_MAP.get_cloned(handle)?;
    let message = proof.mark_presentation_sent()?;
    send_message(connection_handle, message).await?;
    HANDLE_MAP.insert(handle, proof)
}

pub fn generate_reject_proof_msg(_handle: u32) -> LibvcxResult<String> {
    Err(LibvcxError::from_msg(
        LibvcxErrorKind::ActionNotSupported,
        "Action generate_reject_proof_msg is not implemented for V3 disclosed proof.",
    ))
}

pub async fn reject_proof(handle: u32, connection_handle: u32) -> LibvcxResult<()> {
    info!(
        "reject_proof >> handle: {}, connection_handle: {}",
        handle, connection_handle
    );
    let mut proof = HANDLE_MAP.get_cloned(handle)?;
    let message = proof
        .decline_presentation_request(
            Some(String::from("Presentation Request was rejected")),
            None,
        )
        .await?;
    send_message(connection_handle, message).await?;
    HANDLE_MAP.insert(handle, proof)
}

pub async fn generate_proof(
    handle: u32,
    credentials: &str,
    self_attested_attrs: &str,
) -> LibvcxResult<()> {
    let mut proof = HANDLE_MAP.get_cloned(handle)?;
    proof
        .generate_presentation(
            get_main_wallet()?.as_ref(),
            get_main_ledger_read()?.as_ref(),
            get_main_anoncreds()?.as_ref(),
            serde_json::from_str(credentials)?,
            serde_json::from_str(self_attested_attrs)?,
        )
        .await?;
    HANDLE_MAP.insert(handle, proof)
}

pub async fn decline_presentation_request(
    handle: u32,
    connection_handle: u32,
    reason: Option<&str>,
    proposal: Option<&str>,
) -> LibvcxResult<()> {
    let mut proof = HANDLE_MAP.get_cloned(handle)?;
    let message = proof
        .decline_presentation_request(
            reason.map(|s| s.to_string()),
            proposal.map(|s| s.to_string()),
        )
        .await?;
    send_message(connection_handle, message).await?;
    HANDLE_MAP.insert(handle, proof)
}

pub async fn retrieve_credentials(handle: u32) -> LibvcxResult<String> {
    let proof = HANDLE_MAP.get_cloned(handle)?;
    let retrieved_creds = proof
        .retrieve_credentials(get_main_wallet()?.as_ref(), get_main_anoncreds()?.as_ref())
        .await?;

    Ok(serde_json::to_string(&retrieved_creds)?)
}

pub fn get_proof_request_data(handle: u32) -> LibvcxResult<String> {
    HANDLE_MAP.get(handle, |proof| {
        proof.presentation_request_data().map_err(|err| err.into())
    })
}

pub fn get_proof_request_attachment(handle: u32) -> LibvcxResult<String> {
    HANDLE_MAP.get(handle, |proof| {
        proof
            .get_proof_request_attachment()
            .map_err(|err| err.into())
    })
}

pub fn is_valid_handle(handle: u32) -> bool {
    HANDLE_MAP.has_handle(handle)
}

pub fn get_thread_id(handle: u32) -> LibvcxResult<String> {
    HANDLE_MAP.get(handle, |proof| {
        proof.get_thread_id().map_err(|err| err.into())
    })
}

async fn get_proof_request(connection_handle: u32, msg_id: &str) -> LibvcxResult<String> {
    let presentation_request = {
        trace!(
            "Prover::get_presentation_request >>> connection_handle: {:?}, msg_id: {:?}",
            connection_handle,
            msg_id
        );

        let message = mediated_connection::get_message_by_id(connection_handle, msg_id).await?;

        match message {
            AriesMessage::PresentProof(PresentProof::V1(PresentProofV1::RequestPresentation(
                presentation_request,
            ))) => presentation_request,
            msg => {
                return Err(LibvcxError::from_msg(
                    LibvcxErrorKind::InvalidMessages,
                    format!("Message of different type was received: {:?}", msg),
                ));
            }
        }
    };
    serde_json::to_string_pretty(&presentation_request).map_err(|err| {
        LibvcxError::from_msg(
            LibvcxErrorKind::InvalidJson,
            format!("Cannot serialize message: {}", err),
        )
    })
}

pub async fn get_proof_request_messages(connection_handle: u32) -> LibvcxResult<String> {
    trace!(
        "get_proof_request_messages >>> connection_handle: {}",
        connection_handle
    );

    let presentation_requests: Vec<AriesMessage> =
        mediated_connection::get_messages(connection_handle)
            .await?
            .into_iter()
            .filter_map(|(_, message)| match message {
                AriesMessage::PresentProof(PresentProof::V1(
                    PresentProofV1::RequestPresentation(_),
                )) => Some(message),
                _ => None,
            })
            .collect();

    Ok(json!(presentation_requests).to_string())
}

pub fn get_source_id(handle: u32) -> LibvcxResult<String> {
    HANDLE_MAP
        .get(handle, |proof| Ok(proof.get_source_id()))
        .map_err(|e| LibvcxError::from_msg(LibvcxErrorKind::InvalidProofHandle, e.to_string()))
}

pub fn get_presentation_status(handle: u32) -> LibvcxResult<u32> {
    HANDLE_MAP.get(handle, |proof| Ok(proof.presentation_status()))
}

#[cfg(test)]

mod tests {
    extern crate serde_json;

    use serde_json::Value;
    use test_utils::{
        constants::V3_OBJECT_SERIALIZE_VERSION, devsetup::SetupMocks,
        mockdata::mockdata_proof::ARIES_PROOF_REQUEST_PRESENTATION,
    };

    use super::*;
    use crate::aries_vcx::protocols::proof_presentation::prover::state_machine::ProverState;

    async fn _get_proof_request_messages(connection_h: u32) -> String {
        let requests = get_proof_request_messages(connection_h).await.unwrap();
        let requests: Value = serde_json::from_str(&requests).unwrap();
        serde_json::to_string(&requests[0]).unwrap()
    }

    #[tokio::test]
    async fn test_vcx_disclosed_proof_release() {
        let _setup = SetupMocks::init();
        let handle =
            create_with_proof_request("TEST_CREDENTIAL", ARIES_PROOF_REQUEST_PRESENTATION).unwrap();
        release(handle).unwrap();
        assert_eq!(
            to_string(handle).unwrap_err().kind,
            LibvcxErrorKind::InvalidHandle
        )
    }

    #[tokio::test]
    async fn test_create_proof() {
        let _setup = SetupMocks::init();

        assert!(create_with_proof_request("1", ARIES_PROOF_REQUEST_PRESENTATION).unwrap() > 0);
    }

    #[tokio::test]
    async fn test_create_fails() {
        let _setup = SetupMocks::init();

        assert_eq!(
            create_with_proof_request("1", "{}").unwrap_err().kind(),
            LibvcxErrorKind::InvalidJson
        );
    }

    #[tokio::test]
    async fn get_state_test() {
        let _setup = SetupMocks::init();

        let handle = create_with_proof_request("id", ARIES_PROOF_REQUEST_PRESENTATION).unwrap();
        assert_eq!(
            ProverState::PresentationRequestReceived as u32,
            get_state(handle).unwrap()
        )
    }

    #[tokio::test]
    async fn to_string_test() {
        let _setup = SetupMocks::init();

        let handle = create_with_proof_request("id", ARIES_PROOF_REQUEST_PRESENTATION).unwrap();

        let serialized = to_string(handle).unwrap();
        let j: Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(j["version"], V3_OBJECT_SERIALIZE_VERSION);

        let handle_2 = from_string(&serialized).unwrap();
        assert_ne!(handle, handle_2);
    }

    #[tokio::test]
    async fn test_deserialize_fails() {
        let _setup = SetupMocks::init();

        assert_eq!(
            from_string("{}").unwrap_err().kind(),
            LibvcxErrorKind::InvalidJson
        );
    }

    #[tokio::test]
    async fn test_deserialize_succeeds_with_self_attest_allowed() {
        let _setup = SetupMocks::init();

        let handle = create_with_proof_request("id", ARIES_PROOF_REQUEST_PRESENTATION).unwrap();

        let serialized = to_string(handle).unwrap();
        from_string(&serialized).unwrap();
    }
}
