import type { GameSigningDelivery } from '../types';
import { invokeCommand } from './command';

export function approveGameSigningRequest(approvalId: string): Promise<GameSigningDelivery> {
  return invokeCommand('approve_game_signing_request', { approvalId });
}
