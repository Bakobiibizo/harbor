//! Tauri commands for voice calling

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

use crate::error::AppError;
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
    pub reason: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// Start a call (create an offer)
#[tauri::command]
pub async fn start_call(
    calling_service: State<'_, Arc<CallingService>>,
    callee_peer_id: String,
    sdp: String,
) -> Result<OfferResult, AppError> {
    let offer = calling_service.create_offer(&callee_peer_id, &sdp)?;

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
    call_id: String,
    caller_peer_id: String,
    sdp: String,
) -> Result<AnswerResult, AppError> {
    let answer = calling_service.create_answer(&call_id, &caller_peer_id, &sdp)?;

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
    call_id: String,
    candidate: String,
    sdp_mid: Option<String>,
    sdp_mline_index: Option<u32>,
) -> Result<IceResult, AppError> {
    let ice = calling_service.create_ice_candidate(
        &call_id,
        &candidate,
        sdp_mid.as_deref(),
        sdp_mline_index,
    )?;

    Ok(IceResult {
        call_id: ice.call_id,
        sender_peer_id: ice.sender_peer_id,
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
    call_id: String,
    reason: Option<String>,
) -> Result<HangupResult, AppError> {
    let reason = reason.unwrap_or_else(|| "normal".to_string());
    let hangup = calling_service.create_hangup(&call_id, &reason)?;

    Ok(HangupResult {
        call_id: hangup.call_id,
        sender_peer_id: hangup.sender_peer_id,
        reason: hangup.reason,
        timestamp: hangup.timestamp,
        signature: hangup.signature,
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
