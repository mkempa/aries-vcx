use std::sync::Arc;

use aries_vcx::{
    common::proofs::proof_request::PresentationRequestData,
    handlers::proof_presentation::verifier::Verifier,
    messages::{
        msg_fields::protocols::present_proof::v1::{
            present::PresentationV1, propose::ProposePresentationV1,
        },
        AriesMessage,
    },
    protocols::{
        proof_presentation::verifier::{
            state_machine::VerifierState, verification_status::PresentationVerificationStatus,
        },
        SendClosure,
    },
};
use aries_vcx_core::{
    anoncreds::credx_anoncreds::IndyCredxAnonCreds, ledger::indy_vdr_ledger::DefaultIndyLedgerRead,
    wallet::indy::IndySdkWallet,
};

use super::connection::ServiceConnections;
use crate::{
    error::*,
    http::VcxHttpClient,
    storage::{object_cache::ObjectCache, Storage},
};

#[derive(Clone)]
struct VerifierWrapper {
    verifier: Verifier,
    connection_id: String,
}

impl VerifierWrapper {
    pub fn new(verifier: Verifier, connection_id: &str) -> Self {
        Self {
            verifier,
            connection_id: connection_id.to_string(),
        }
    }
}

pub struct ServiceVerifier {
    ledger_read: Arc<DefaultIndyLedgerRead>,
    anoncreds: IndyCredxAnonCreds,
    wallet: Arc<IndySdkWallet>,
    verifiers: ObjectCache<VerifierWrapper>,
    service_connections: Arc<ServiceConnections>,
}

impl ServiceVerifier {
    pub fn new(
        ledger_read: Arc<DefaultIndyLedgerRead>,
        anoncreds: IndyCredxAnonCreds,
        wallet: Arc<IndySdkWallet>,
        service_connections: Arc<ServiceConnections>,
    ) -> Self {
        Self {
            service_connections,
            verifiers: ObjectCache::new("verifiers"),
            ledger_read,
            anoncreds,
            wallet,
        }
    }

    pub async fn send_proof_request(
        &self,
        connection_id: &str,
        request: PresentationRequestData,
        proposal: Option<ProposePresentationV1>,
    ) -> AgentResult<String> {
        let connection = self.service_connections.get_by_id(connection_id)?;
        let mut verifier = if let Some(proposal) = proposal {
            Verifier::create_from_proposal("", &proposal)?
        } else {
            Verifier::create_from_request("".to_string(), &request)?
        };

        let wallet = self.wallet.as_ref();

        let send_closure: SendClosure = Box::new(|msg: AriesMessage| {
            Box::pin(async move { connection.send_message(wallet, &msg, &VcxHttpClient).await })
        });

        let message = verifier.mark_presentation_request_sent()?;
        send_closure(message.into()).await?;
        self.verifiers.insert(
            &verifier.get_thread_id()?,
            VerifierWrapper::new(verifier, connection_id),
        )
    }

    pub fn get_presentation_status(
        &self,
        thread_id: &str,
    ) -> AgentResult<PresentationVerificationStatus> {
        let VerifierWrapper { verifier, .. } = self.verifiers.get(thread_id)?;
        Ok(verifier.get_verification_status())
    }

    pub async fn verify_presentation(
        &self,
        thread_id: &str,
        presentation: PresentationV1,
    ) -> AgentResult<()> {
        let VerifierWrapper {
            mut verifier,
            connection_id,
        } = self.verifiers.get(thread_id)?;
        let connection = self.service_connections.get_by_id(&connection_id)?;
        let wallet = self.wallet.as_ref();

        let send_closure: SendClosure = Box::new(|msg: AriesMessage| {
            Box::pin(async move { connection.send_message(wallet, &msg, &VcxHttpClient).await })
        });

        let message = verifier
            .verify_presentation(self.ledger_read.as_ref(), &self.anoncreds, presentation)
            .await?;
        send_closure(message).await?;
        self.verifiers
            .insert(thread_id, VerifierWrapper::new(verifier, &connection_id))?;
        Ok(())
    }

    pub fn get_state(&self, thread_id: &str) -> AgentResult<VerifierState> {
        let VerifierWrapper { verifier, .. } = self.verifiers.get(thread_id)?;
        Ok(verifier.get_state())
    }

    pub fn exists_by_id(&self, thread_id: &str) -> bool {
        self.verifiers.contains_key(thread_id)
    }
}
