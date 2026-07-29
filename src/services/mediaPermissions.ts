import { callFailureFrom, type CallFailureCode } from '../utils/callErrors';

export type MediaPermissionState =
  | 'ready'
  | 'permission_denied'
  | 'missing_device'
  | 'missing_media_api'
  | 'unknown';

export interface MediaPermissionResult {
  microphone: MediaPermissionState;
  camera: MediaPermissionState;
  audioInputCount: number | null;
  videoInputCount: number | null;
}

type CallMediaDevices = Partial<Pick<MediaDevices, 'getUserMedia' | 'enumerateDevices'>>;
type AvailableCallMediaDevices = Required<Pick<MediaDevices, 'getUserMedia'>> &
  Partial<Pick<MediaDevices, 'enumerateDevices'>>;

function availableMediaDevices(configured?: CallMediaDevices): AvailableCallMediaDevices | null {
  const devices = configured ?? globalThis.navigator?.mediaDevices;
  return devices?.getUserMedia ? (devices as AvailableCallMediaDevices) : null;
}

function permissionState(code: CallFailureCode): MediaPermissionState {
  switch (code) {
    case 'permission_denied':
    case 'missing_device':
    case 'missing_media_api':
      return code;
    default:
      return 'unknown';
  }
}

async function requestKind(
  devices: AvailableCallMediaDevices,
  kind: 'microphone' | 'camera',
): Promise<MediaPermissionState> {
  try {
    const stream = await devices.getUserMedia(
      kind === 'microphone' ? { audio: true, video: false } : { audio: false, video: true },
    );
    stream.getTracks().forEach((track) => track.stop());
    return 'ready';
  } catch (error) {
    return permissionState(callFailureFrom(error, `request-${kind}-access`).code);
  }
}

async function countInputs(
  devices: AvailableCallMediaDevices,
): Promise<Pick<MediaPermissionResult, 'audioInputCount' | 'videoInputCount'>> {
  if (!devices.enumerateDevices) {
    return { audioInputCount: null, videoInputCount: null };
  }
  try {
    const entries = await devices.enumerateDevices();
    return {
      audioInputCount: entries.filter((entry) => entry.kind === 'audioinput').length,
      videoInputCount: entries.filter((entry) => entry.kind === 'videoinput').length,
    };
  } catch {
    // WebKit can reject enumeration until permission is decided. Calls should still
    // request access normally, so an enumeration failure is diagnostic, not fatal.
    return { audioInputCount: null, videoInputCount: null };
  }
}

/**
 * Explicitly requests call-media access and immediately releases all tracks.
 * Keeping this behind a user gesture makes the macOS privacy prompt predictable.
 */
export async function requestCallMediaAccess(
  configured?: CallMediaDevices,
): Promise<MediaPermissionResult> {
  const devices = availableMediaDevices(configured);
  if (!devices) {
    return {
      microphone: 'missing_media_api',
      camera: 'missing_media_api',
      audioInputCount: null,
      videoInputCount: null,
    };
  }

  const microphone = await requestKind(devices, 'microphone');
  const camera = await requestKind(devices, 'camera');
  const counts = await countInputs(devices);
  return { microphone, camera, ...counts };
}
