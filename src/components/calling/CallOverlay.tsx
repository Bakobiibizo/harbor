import { useEffect, useMemo, useRef } from 'react';
import { PhoneIcon, XIcon } from '../icons';
import { useCallingStore, useContactsStore } from '../../stores';
import { safePeerLabel } from '../../utils/relayName';
import type {
  AudioCallRuntimeState,
  GroupCallParticipantSnapshot,
} from '../../services/callingRuntime';

function formatState(state: AudioCallRuntimeState, isVideo: boolean): string {
  const media = isVideo ? 'video' : 'voice';
  switch (state) {
    case 'requesting_microphone':
      return isVideo ? 'Requesting camera and microphone…' : 'Requesting microphone…';
    case 'ringing':
      return 'Ringing…';
    case 'incoming':
      return `Incoming ${media} call`;
    case 'connecting':
      return `Connecting ${isVideo ? 'media' : 'audio'}…`;
    case 'connected':
      return `${isVideo ? 'Video' : 'Voice'} call connected`;
    case 'failed':
      return 'Call failed';
    case 'ended':
      return 'Call ended';
    default:
      return `${isVideo ? 'Video' : 'Voice'} call`;
  }
}

function useVideoElement(stream: MediaStream | null) {
  const ref = useRef<HTMLVideoElement>(null);
  useEffect(() => {
    if (ref.current) {
      ref.current.srcObject = stream;
    }
  }, [stream]);
  return ref;
}

function ParticipantTile({
  participant,
  name,
}: {
  participant: GroupCallParticipantSnapshot;
  name: string;
}) {
  const videoRef = useVideoElement(participant.remoteVideoStream);
  const showVideo = participant.mediaMode === 'video' && participant.remoteVideoAvailable;
  return (
    <div
      className="relative min-h-28 rounded-xl overflow-hidden bg-black/60 border"
      style={{
        borderColor: participant.activeSpeaker
          ? 'hsl(var(--harbor-primary))'
          : 'hsl(var(--harbor-border-subtle))',
      }}
    >
      {showVideo ? (
        <video
          ref={videoRef}
          autoPlay
          playsInline
          className="w-full h-full min-h-28 object-cover"
        />
      ) : (
        <div className="min-h-28 flex items-center justify-center text-sm text-white/70 px-3 text-center">
          {participant.state === 'failed'
            ? 'Media unavailable'
            : participant.mediaMode === 'video'
              ? 'Waiting for video'
              : 'Audio only'}
        </div>
      )}
      <div className="absolute inset-x-0 bottom-0 px-2 py-1 text-xs bg-black/60 text-white flex justify-between gap-2">
        <span className="truncate">{name}</span>
        <span>
          {participant.cameraEnabled ? 'Camera on' : 'Camera off'} · {participant.state}
        </span>
      </div>
    </div>
  );
}

export function CallOverlay() {
  const snapshot = useCallingStore((state) => state.runtimeSnapshot);
  const groupSnapshot = useCallingStore((state) => state.groupRuntimeSnapshot);
  const error = useCallingStore((state) => state.error);
  const failure = useCallingStore((state) => state.failure);
  const startError = failure?.message ?? snapshot.error ?? error;
  const acceptIncomingCall = useCallingStore((state) => state.acceptIncomingCall);
  const acceptIncomingGroupCall = useCallingStore((state) => state.acceptIncomingGroupCall);
  const declineIncomingCall = useCallingStore((state) => state.declineIncomingCall);
  const declineIncomingGroupCall = useCallingStore((state) => state.declineIncomingGroupCall);
  const hangupActiveCall = useCallingStore((state) => state.hangupActiveCall);
  const leaveGroupCall = useCallingStore((state) => state.leaveGroupCall);
  const dismissCallUi = useCallingStore((state) => state.dismissCallUi);
  const setCameraEnabled = useCallingStore((state) => state.setCameraEnabled);
  const setGroupMuted = useCallingStore((state) => state.setGroupMuted);
  const setGroupCameraEnabled = useCallingStore((state) => state.setGroupCameraEnabled);
  const contacts = useContactsStore((state) => state.contacts);
  const localVideoRef = useVideoElement(snapshot.localVideoStream);
  const remoteVideoRef = useVideoElement(snapshot.remoteVideoStream);

  const peerName = useMemo(() => {
    const peerId = snapshot.peerId;
    if (!peerId) return 'Unverified Harbor user';
    const contact = contacts.find((item) => item.peerId === peerId);
    return safePeerLabel(peerId, contact?.verifiedQualifiedName);
  }, [contacts, snapshot.peerId]);

  if (groupSnapshot.state !== 'idle') {
    const isTerminalGroup = groupSnapshot.state === 'ended' || groupSnapshot.state === 'failed';
    const isIncomingGroup =
      groupSnapshot.state === 'ringing' &&
      groupSnapshot.participants.every((participant) => participant.state === 'invited');
    const contactName = (peerId: string) => {
      const contact = contacts.find((item) => item.peerId === peerId);
      return safePeerLabel(peerId, contact?.verifiedQualifiedName);
    };
    return (
      <div className="fixed inset-x-4 bottom-4 z-[150] sm:left-auto sm:w-[36rem]">
        <div
          role="dialog"
          aria-live="polite"
          aria-label="Group call"
          className="rounded-2xl border p-4 shadow-2xl backdrop-blur"
          style={{
            background: 'hsl(var(--harbor-bg-elevated) / 0.96)',
            borderColor: 'hsl(var(--harbor-border-subtle))',
            color: 'hsl(var(--harbor-text-primary))',
          }}
        >
          <div className="flex items-start justify-between gap-3">
            <div>
              <p className="font-semibold">
                Group {groupSnapshot.mediaMode === 'video' ? 'video' : 'voice'} call
              </p>
              <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                {groupSnapshot.participantCount}/{groupSnapshot.maxParticipants} participants ·{' '}
                {groupSnapshot.state}
              </p>
              {groupSnapshot.error && (
                <div className="text-xs mt-1" style={{ color: 'hsl(var(--harbor-error))' }}>
                  <p>{failure?.message ?? groupSnapshot.error}</p>
                  {failure?.recovery && <p className="mt-1">{failure.recovery}</p>}
                </div>
              )}
            </div>
            <span
              className="text-xs px-2 py-1 rounded-full"
              style={{
                background: 'hsl(var(--harbor-surface-2))',
                color: 'hsl(var(--harbor-text-secondary))',
              }}
            >
              Mesh
            </span>
          </div>

          <div className="mt-4 grid grid-cols-1 sm:grid-cols-2 gap-2">
            {groupSnapshot.participants.map((participant) => (
              <ParticipantTile
                key={participant.peerId}
                participant={participant}
                name={contactName(participant.peerId)}
              />
            ))}
          </div>

          {groupSnapshot.participants.some((participant) => participant.error) && (
            <div className="mt-3 text-xs" style={{ color: 'hsl(var(--harbor-warning))' }}>
              Some participants have degraded media; remaining mesh connections continue.
            </div>
          )}

          <div className="mt-4 flex justify-end gap-2">
            {isIncomingGroup && (
              <>
                <button
                  onClick={() => void declineIncomingGroupCall()}
                  className="px-4 py-2 rounded-lg text-sm font-medium"
                  style={{ color: 'hsl(var(--harbor-error))' }}
                >
                  Decline
                </button>
                <button
                  onClick={() => void acceptIncomingGroupCall()}
                  className="px-4 py-2 rounded-lg text-sm font-medium"
                  style={{
                    background: 'hsl(var(--harbor-accent))',
                    color: 'hsl(var(--harbor-bg-primary))',
                  }}
                >
                  Accept
                </button>
              </>
            )}
            {!isTerminalGroup && !isIncomingGroup && (
              <>
                <button
                  aria-pressed={groupSnapshot.localMuted}
                  onClick={() => void setGroupMuted(!groupSnapshot.localMuted)}
                  className="px-4 py-2 rounded-lg text-sm font-medium transition-colors"
                  style={{
                    background: 'hsl(var(--harbor-surface-2))',
                    color: 'hsl(var(--harbor-text-primary))',
                  }}
                >
                  {groupSnapshot.localMuted ? 'Unmute' : 'Mute'}
                </button>
                {groupSnapshot.mediaMode === 'video' && (
                  <button
                    aria-pressed={groupSnapshot.localCameraEnabled}
                    onClick={() => void setGroupCameraEnabled(!groupSnapshot.localCameraEnabled)}
                    className="px-4 py-2 rounded-lg text-sm font-medium transition-colors"
                    style={{
                      background: 'hsl(var(--harbor-surface-2))',
                      color: 'hsl(var(--harbor-text-primary))',
                    }}
                  >
                    {groupSnapshot.localCameraEnabled ? 'Camera off' : 'Camera on'}
                  </button>
                )}
              </>
            )}
            {!isIncomingGroup && (
              <button
                onClick={() => {
                  if (isTerminalGroup) dismissCallUi();
                  else void leaveGroupCall('normal');
                }}
                className="px-4 py-2 rounded-lg text-sm font-medium transition-colors flex items-center gap-2"
                style={{
                  background: isTerminalGroup
                    ? 'hsl(var(--harbor-surface-2))'
                    : 'hsl(var(--harbor-error) / 0.16)',
                  color: isTerminalGroup
                    ? 'hsl(var(--harbor-text-secondary))'
                    : 'hsl(var(--harbor-error))',
                }}
              >
                {isTerminalGroup ? <XIcon className="w-4 h-4" /> : null}
                {isTerminalGroup ? 'Dismiss' : 'Leave'}
              </button>
            )}
          </div>
        </div>
      </div>
    );
  }

  if (snapshot.state === 'idle') return null;

  const isIncoming = snapshot.state === 'incoming';
  const isTerminal = snapshot.state === 'ended' || snapshot.state === 'failed';
  const isVideo = snapshot.videoRequested || snapshot.mediaMode === 'video';
  const showVideo = isVideo && !isIncoming && !isTerminal;

  return (
    <div className="fixed inset-x-4 bottom-4 z-[150] sm:left-auto sm:w-[28rem]">
      <div
        className="rounded-2xl border p-4 shadow-2xl backdrop-blur"
        style={{
          background: 'hsl(var(--harbor-bg-elevated) / 0.96)',
          borderColor: 'hsl(var(--harbor-border-subtle))',
          color: 'hsl(var(--harbor-text-primary))',
        }}
      >
        {showVideo && (
          <div className="mb-4 grid grid-cols-3 gap-2 overflow-hidden rounded-xl">
            <div className="col-span-2 relative aspect-video bg-black/60 rounded-xl overflow-hidden">
              {snapshot.remoteVideoStream && snapshot.remoteVideoAvailable ? (
                <video
                  ref={remoteVideoRef}
                  autoPlay
                  playsInline
                  className="w-full h-full object-cover"
                />
              ) : (
                <div className="w-full h-full flex items-center justify-center text-sm text-white/70">
                  Waiting for remote video
                </div>
              )}
            </div>
            <div className="relative aspect-video bg-black/60 rounded-xl overflow-hidden">
              {snapshot.localVideoStream && snapshot.localVideoEnabled ? (
                <video
                  ref={localVideoRef}
                  autoPlay
                  muted
                  playsInline
                  className="w-full h-full object-cover scale-x-[-1]"
                />
              ) : (
                <div className="w-full h-full flex items-center justify-center text-xs text-white/70 px-2 text-center">
                  Camera off
                </div>
              )}
            </div>
          </div>
        )}

        <div className="flex items-start gap-3">
          <div
            className="w-11 h-11 rounded-full flex items-center justify-center flex-shrink-0"
            style={{
              background: isIncoming
                ? 'hsl(var(--harbor-primary) / 0.16)'
                : 'hsl(var(--harbor-success) / 0.16)',
              color: isIncoming ? 'hsl(var(--harbor-primary))' : 'hsl(var(--harbor-success))',
            }}
          >
            <PhoneIcon className="w-5 h-5" />
          </div>
          <div className="min-w-0 flex-1">
            <p className="font-semibold truncate">{peerName}</p>
            <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              {formatState(snapshot.state, isVideo)}
            </p>
            {snapshot.cameraError && (
              <p className="text-xs mt-1" style={{ color: 'hsl(var(--harbor-warning))' }}>
                Camera unavailable; continuing with audio. {snapshot.cameraError}
              </p>
            )}
            {snapshot.ice?.error && (
              <p className="text-xs mt-1" style={{ color: 'hsl(var(--harbor-warning))' }}>
                {snapshot.ice.error.message}
              </p>
            )}
            {startError && (
              <div className="text-xs mt-1" style={{ color: 'hsl(var(--harbor-error))' }}>
                <p>{startError}</p>
                {failure?.recovery && <p className="mt-1">{failure.recovery}</p>}
              </div>
            )}
          </div>
        </div>

        <div className="mt-4 flex justify-end gap-2">
          {showVideo && (
            <button
              onClick={() => void setCameraEnabled(!snapshot.localVideoEnabled)}
              className="px-4 py-2 rounded-lg text-sm font-medium transition-colors"
              style={{
                background: 'hsl(var(--harbor-surface-2))',
                color: 'hsl(var(--harbor-text-primary))',
              }}
            >
              {snapshot.localVideoEnabled ? 'Camera off' : 'Camera on'}
            </button>
          )}
          {isIncoming && (
            <button
              onClick={() => void acceptIncomingCall()}
              className="px-4 py-2 rounded-lg text-sm font-medium transition-colors"
              style={{
                background: 'hsl(var(--harbor-success))',
                color: 'white',
              }}
            >
              Answer
            </button>
          )}
          {isIncoming ? (
            <button
              onClick={() => void declineIncomingCall()}
              className="px-4 py-2 rounded-lg text-sm font-medium transition-colors"
              style={{
                background: 'hsl(var(--harbor-error) / 0.16)',
                color: 'hsl(var(--harbor-error))',
              }}
            >
              Decline
            </button>
          ) : (
            <button
              onClick={() => {
                if (isTerminal) {
                  dismissCallUi();
                } else {
                  void hangupActiveCall('normal');
                }
              }}
              className="px-4 py-2 rounded-lg text-sm font-medium transition-colors flex items-center gap-2"
              style={{
                background: isTerminal
                  ? 'hsl(var(--harbor-surface-2))'
                  : 'hsl(var(--harbor-error) / 0.16)',
                color: isTerminal
                  ? 'hsl(var(--harbor-text-secondary))'
                  : 'hsl(var(--harbor-error))',
              }}
            >
              {isTerminal ? <XIcon className="w-4 h-4" /> : null}
              {isTerminal ? 'Dismiss' : 'Hang up'}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
