import { invokeCommand } from './command';

export interface MessagingPrivacyPolicy {
  readReceiptsEnabled: boolean;
}

export function getMessagingPrivacyPolicy(): Promise<MessagingPrivacyPolicy> {
  return invokeCommand('get_messaging_privacy_policy');
}

export function setReadReceiptsEnabled(enabled: boolean): Promise<MessagingPrivacyPolicy> {
  return invokeCommand('set_read_receipts_enabled', { enabled });
}
