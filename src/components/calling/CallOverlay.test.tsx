import { render, screen, fireEvent } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { CallOverlay } from './CallOverlay';
import { useCallingStore, useContactsStore } from '../../stores';
import type { GroupCallRuntimeSnapshot } from '../../services/callingRuntime';

function contact(peerId: string, displayName: string, verifiedQualifiedName?: string) {
  return {
    id: 1,
    peerId,
    publicKey: 'pub',
    x25519Public: 'xpub',
    displayName,
    verifiedQualifiedName,
    avatarHash: null,
    bio: null,
    isBlocked: false,
    trustLevel: 1,
    lastSeenAt: null,
    addedAt: 1,
    updatedAt: 1,
  };
}

function groupSnapshot(
  overrides: Partial<GroupCallRuntimeSnapshot> = {},
): GroupCallRuntimeSnapshot {
  return {
    state: 'degraded',
    roomId: 'room-1',
    topology: 'relay_assisted_mesh_v1',
    maxParticipants: 4,
    mediaMode: 'video',
    localPeerId: 'peer-local',
    localMuted: false,
    localCameraEnabled: true,
    participantCount: 3,
    error: null,
    participants: [
      {
        peerId: 'peer-a',
        state: 'connected',
        callId: 'call-a',
        mediaMode: 'video',
        muted: false,
        cameraEnabled: true,
        localVideoStream: null,
        remoteVideoStream: null,
        remoteVideoAvailable: false,
        activeSpeaker: true,
        error: null,
        terminalReason: null,
        ice: null,
      },
      {
        peerId: 'peer-b',
        state: 'failed',
        callId: null,
        mediaMode: 'video',
        muted: false,
        cameraEnabled: false,
        localVideoStream: null,
        remoteVideoStream: null,
        remoteVideoAvailable: false,
        activeSpeaker: false,
        error: 'ICE failed',
        terminalReason: 'ice_failed',
        ice: null,
      },
    ],
    ...overrides,
  };
}

describe('CallOverlay group call UI', () => {
  beforeEach(() => {
    useCallingStore.getState().reset();
    useContactsStore.setState({
      contacts: [contact('peer-a', 'Alice', '@alice@relay.test'), contact('peer-b', 'Bob')],
    });
  });

  it('renders participant tiles, layout state, and degraded media without hiding leave controls', () => {
    const leaveGroupCall = vi.fn(async () => undefined);
    useCallingStore.setState({
      groupRuntimeSnapshot: groupSnapshot(),
      leaveGroupCall,
    });

    render(<CallOverlay />);

    expect(screen.getByRole('dialog', { name: 'Group call' })).toBeInTheDocument();
    expect(screen.getByText('Group video call')).toBeInTheDocument();
    expect(screen.getByText('3/4 participants · degraded')).toBeInTheDocument();
    expect(screen.getByText('@alice@relay.test')).toBeInTheDocument();
    expect(screen.getByText('Peer peer-b… (unverified)')).toBeInTheDocument();
    expect(screen.queryByText('Alice')).not.toBeInTheDocument();
    expect(screen.queryByText('Bob')).not.toBeInTheDocument();
    expect(
      screen.getByText(
        'Some participants have degraded media; remaining mesh connections continue.',
      ),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Leave' }));
    expect(leaveGroupCall).toHaveBeenCalledWith('normal');
  });

  it('exposes accessible mute and camera controls for local group media state', () => {
    const setGroupMuted = vi.fn(async () => undefined);
    const setGroupCameraEnabled = vi.fn(async () => undefined);
    useCallingStore.setState({
      groupRuntimeSnapshot: groupSnapshot({ localMuted: true, localCameraEnabled: false }),
      setGroupMuted,
      setGroupCameraEnabled,
    });

    render(<CallOverlay />);

    fireEvent.click(screen.getByRole('button', { name: 'Unmute' }));
    fireEvent.click(screen.getByRole('button', { name: 'Camera on' }));

    expect(setGroupMuted).toHaveBeenCalledWith(false);
    expect(setGroupCameraEnabled).toHaveBeenCalledWith(true);
  });
});
