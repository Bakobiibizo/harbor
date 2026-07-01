//! Tauri commands for voice calling

use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tauri::State;

use crate::commands::network::NetworkState;
use crate::error::AppError;
use crate::p2p::protocols::signaling::{
    SignalingAnswer, SignalingEnvelope, SignalingHangup, SignalingIce, SignalingOffer,
    SignalingPayload,
};
use crate::services::calling_service::IncomingIceParams;
use crate::services::CallingService;

/// Offer result for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfferResult {
    pub call_id: String,
    pub caller_peer_id: String,
    pub callee_peer_id: String,
    pub sdp: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// Answer result for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerResult {
    pub call_id: String,
    pub caller_peer_id: String,
    pub callee_peer_id: String,
    pub sdp: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// ICE candidate result for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IceResult {
    pub call_id: String,
    pub sender_peer_id: String,
    pub target_peer_id: String,
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_mline_index: Option<u32>,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// Hangup result for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HangupResult {
    pub call_id: String,
    pub sender_peer_id: String,
    pub target_peer_id: String,
    pub reason: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

async fn transmit_signaling(
    network: &NetworkState,
    target_peer_id: &str,
    envelope: SignalingEnvelope,
) -> Result<(), AppError> {
    let libp2p_peer_id = PeerId::from_str(target_peer_id)
        .map_err(|e| AppError::Validation(format!("Invalid peer ID: {}", e)))?;
    let handle = network.get_handle().await?;
    handle
        .send_signaling(libp2p_peer_id, envelope)
        .await
        .map_err(map_signaling_transport_error)
}

fn map_signaling_transport_error(error: AppError) -> AppError {
    match error {
        AppError::Network(message) if message.contains("SIGNALING_PEER_OFFLINE") => {
            AppError::NetworkPeerUnreachable("Peer is offline or not connected".to_string())
        }
        AppError::Network(message) if message.contains("SIGNALING_NETWORK_FAILURE") => {
            AppError::NetworkConnectionFailed(message)
        }
        AppError::Network(message) => AppError::Network(message),
        other => other,
    }
}

/// Start a call (create an offer)
#[tauri::command]
pub async fn start_call(
    calling_service: State<'_, Arc<CallingService>>,
    network: State<'_, NetworkState>,
    callee_peer_id: String,
    sdp: String,
) -> Result<OfferResult, AppError> {
    let offer = calling_service.create_offer(&callee_peer_id, &sdp)?;
    let envelope = SignalingEnvelope {
        sender_peer_id: offer.caller_peer_id.clone(),
        recipient_peer_id: offer.callee_peer_id.clone(),
        payload: SignalingPayload::Offer(SignalingOffer {
            call_id: offer.call_id.clone(),
            caller_peer_id: offer.caller_peer_id.clone(),
            callee_peer_id: offer.callee_peer_id.clone(),
            sdp: offer.sdp.clone(),
            timestamp: offer.timestamp,
            signature: offer.signature.clone(),
        }),
    };
    transmit_signaling(&network, &offer.callee_peer_id, envelope).await?;

    Ok(OfferResult {
        call_id: offer.call_id,
        caller_peer_id: offer.caller_peer_id,
        callee_peer_id: offer.callee_peer_id,
        sdp: offer.sdp,
        timestamp: offer.timestamp,
        signature: offer.signature,
    })
}

/// Answer a call
#[tauri::command]
pub async fn answer_call(
    calling_service: State<'_, Arc<CallingService>>,
    network: State<'_, NetworkState>,
    call_id: String,
    caller_peer_id: String,
    sdp: String,
) -> Result<AnswerResult, AppError> {
    let answer = calling_service.create_answer(&call_id, &caller_peer_id, &sdp)?;
    let envelope = SignalingEnvelope {
        sender_peer_id: answer.callee_peer_id.clone(),
        recipient_peer_id: answer.caller_peer_id.clone(),
        payload: SignalingPayload::Answer(SignalingAnswer {
            call_id: answer.call_id.clone(),
            caller_peer_id: answer.caller_peer_id.clone(),
            callee_peer_id: answer.callee_peer_id.clone(),
            sdp: answer.sdp.clone(),
            timestamp: answer.timestamp,
            signature: answer.signature.clone(),
        }),
    };
    transmit_signaling(&network, &answer.caller_peer_id, envelope).await?;

    Ok(AnswerResult {
        call_id: answer.call_id,
        caller_peer_id: answer.caller_peer_id,
        callee_peer_id: answer.callee_peer_id,
        sdp: answer.sdp,
        timestamp: answer.timestamp,
        signature: answer.signature,
    })
}

/// Send an ICE candidate
#[tauri::command]
pub async fn send_ice_candidate(
    calling_service: State<'_, Arc<CallingService>>,
    network: State<'_, NetworkState>,
    call_id: String,
    target_peer_id: String,
    candidate: String,
    sdp_mid: Option<String>,
    sdp_mline_index: Option<u32>,
) -> Result<IceResult, AppError> {
    let ice = calling_service.create_ice_candidate(
        &call_id,
        &target_peer_id,
        &candidate,
        sdp_mid.as_deref(),
        sdp_mline_index,
    )?;
    let envelope = SignalingEnvelope {
        sender_peer_id: ice.sender_peer_id.clone(),
        recipient_peer_id: ice.target_peer_id.clone(),
        payload: SignalingPayload::Ice(SignalingIce {
            call_id: ice.call_id.clone(),
            sender_peer_id: ice.sender_peer_id.clone(),
            candidate: ice.candidate.clone(),
            sdp_mid: ice.sdp_mid.clone(),
            sdp_mline_index: ice.sdp_mline_index,
            timestamp: ice.timestamp,
            signature: ice.signature.clone(),
        }),
    };
    transmit_signaling(&network, &ice.target_peer_id, envelope).await?;

    Ok(IceResult {
        call_id: ice.call_id,
        sender_peer_id: ice.sender_peer_id,
        target_peer_id: ice.target_peer_id,
        candidate: ice.candidate,
        sdp_mid: ice.sdp_mid,
        sdp_mline_index: ice.sdp_mline_index,
        timestamp: ice.timestamp,
        signature: ice.signature,
    })
}

/// Hang up a call
#[tauri::command]
pub async fn hangup_call(
    calling_service: State<'_, Arc<CallingService>>,
    network: State<'_, NetworkState>,
    call_id: String,
    target_peer_id: String,
    reason: Option<String>,
) -> Result<HangupResult, AppError> {
    let reason = reason.unwrap_or_else(|| "normal".to_string());
    let hangup = calling_service.create_hangup(&call_id, &target_peer_id, &reason)?;
    let envelope = SignalingEnvelope {
        sender_peer_id: hangup.sender_peer_id.clone(),
        recipient_peer_id: hangup.target_peer_id.clone(),
        payload: SignalingPayload::Hangup(SignalingHangup {
            call_id: hangup.call_id.clone(),
            sender_peer_id: hangup.sender_peer_id.clone(),
            reason: hangup.reason.clone(),
            timestamp: hangup.timestamp,
            signature: hangup.signature.clone(),
        }),
    };
    transmit_signaling(&network, &hangup.target_peer_id, envelope).await?;

    Ok(HangupResult {
        call_id: hangup.call_id,
        sender_peer_id: hangup.sender_peer_id,
        target_peer_id: hangup.target_peer_id,
        reason: hangup.reason,
        timestamp: hangup.timestamp,
        signature: hangup.signature,
    })
}

/// Decline an incoming call.
#[tauri::command]
pub async fn decline_call(
    calling_service: State<'_, Arc<CallingService>>,
    network: State<'_, NetworkState>,
    call_id: String,
    caller_peer_id: String,
) -> Result<HangupResult, AppError> {
    let decline = calling_service.create_decline(&call_id, &caller_peer_id)?;
    let envelope = SignalingEnvelope {
        sender_peer_id: decline.sender_peer_id.clone(),
        recipient_peer_id: decline.target_peer_id.clone(),
        payload: SignalingPayload::Decline(SignalingHangup {
            call_id: decline.call_id.clone(),
            sender_peer_id: decline.sender_peer_id.clone(),
            reason: decline.reason.clone(),
            timestamp: decline.timestamp,
            signature: decline.signature.clone(),
        }),
    };
    transmit_signaling(&network, &decline.target_peer_id, envelope).await?;

    Ok(HangupResult {
        call_id: decline.call_id,
        sender_peer_id: decline.sender_peer_id,
        target_peer_id: decline.target_peer_id,
        reason: decline.reason,
        timestamp: decline.timestamp,
        signature: decline.signature,
    })
}

/// Send a busy response to an incoming call.
#[tauri::command]
pub async fn busy_call(
    calling_service: State<'_, Arc<CallingService>>,
    network: State<'_, NetworkState>,
    call_id: String,
    caller_peer_id: String,
) -> Result<HangupResult, AppError> {
    let busy = calling_service.create_busy(&call_id, &caller_peer_id)?;
    let envelope = SignalingEnvelope {
        sender_peer_id: busy.sender_peer_id.clone(),
        recipient_peer_id: busy.target_peer_id.clone(),
        payload: SignalingPayload::Busy(SignalingHangup {
            call_id: busy.call_id.clone(),
            sender_peer_id: busy.sender_peer_id.clone(),
            reason: busy.reason.clone(),
            timestamp: busy.timestamp,
            signature: busy.signature.clone(),
        }),
    };
    transmit_signaling(&network, &busy.target_peer_id, envelope).await?;

    Ok(HangupResult {
        call_id: busy.call_id,
        sender_peer_id: busy.sender_peer_id,
        target_peer_id: busy.target_peer_id,
        reason: busy.reason,
        timestamp: busy.timestamp,
        signature: busy.signature,
    })
}

/// Process an incoming offer (validate it)
#[tauri::command]
pub async fn process_offer(
    calling_service: State<'_, Arc<CallingService>>,
    call_id: String,
    caller_peer_id: String,
    callee_peer_id: String,
    sdp: String,
    timestamp: i64,
    signature: Vec<u8>,
) -> Result<(), AppError> {
    calling_service.process_incoming_offer(
        &call_id,
        &caller_peer_id,
        &callee_peer_id,
        &sdp,
        timestamp,
        &signature,
    )
}

/// Process an incoming answer (validate it)
#[tauri::command]
pub async fn process_answer(
    calling_service: State<'_, Arc<CallingService>>,
    call_id: String,
    caller_peer_id: String,
    callee_peer_id: String,
    sdp: String,
    timestamp: i64,
    signature: Vec<u8>,
) -> Result<(), AppError> {
    calling_service.process_incoming_answer(
        &call_id,
        &caller_peer_id,
        &callee_peer_id,
        &sdp,
        timestamp,
        &signature,
    )
}

/// Parameters for processing an incoming ICE candidate
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessIceCandidateParams {
    pub call_id: String,
    pub sender_peer_id: String,
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_mline_index: Option<u32>,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// Process an incoming ICE candidate (validate it)
#[tauri::command]
pub async fn process_ice_candidate(
    calling_service: State<'_, Arc<CallingService>>,
    params: ProcessIceCandidateParams,
) -> Result<(), AppError> {
    calling_service.process_incoming_ice(&IncomingIceParams {
        call_id: &params.call_id,
        sender_peer_id: &params.sender_peer_id,
        candidate: &params.candidate,
        sdp_mid: params.sdp_mid.as_deref(),
        sdp_mline_index: params.sdp_mline_index,
        timestamp: params.timestamp,
        signature: &params.signature,
    })
}

/// Process an incoming hangup (validate it)
#[tauri::command]
pub async fn process_hangup(
    calling_service: State<'_, Arc<CallingService>>,
    call_id: String,
    sender_peer_id: String,
    reason: String,
    timestamp: i64,
    signature: Vec<u8>,
) -> Result<(), AppError> {
    calling_service.process_incoming_hangup(
        &call_id,
        &sender_peer_id,
        &reason,
        timestamp,
        &signature,
    )
}
