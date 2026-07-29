import { invoke } from '@tauri-apps/api/core';
import type {
  AccountInfo,
  DeleteAccountProfileResult,
  AnswerResult,
  BoardInfo,
  BoardPost,
  CallSession,
  Capability,
  Contact,
  ContactDecisionResult,
  ContactRequest,
  AddContactResult,
  Conversation,
  CreateIdentityRequest,
  CreatePostMediaInput,
  CreatePostResult,
  EnsureMediaTransferInput,
  FeedItem,
  GrantResult,
  GroupCallRoom,
  GroupMembershipAction,
  GroupMembershipSignal,
  HangupReason,
  HangupResult,
  IceResult,
  IdentityInfo,
  IdentityBackupExportResult,
  IdentityBackupRestoreResult,
  IdentityInitializationResult,
  MediaCacheDiagnostics,
  MediaCacheSettings,
  MediaAssetInfo,
  MediaTransferState,
  StoredMediaInfo,
  MentionReceipt,
  Message,
  NetworkStats,
  OfferResult,
  PeerInfo,
  PermissionInfo,
  Post,
  PostMedia,
  PostMutationResult,
  PostVisibility,
  PublishMentionedPostRequest,
  PublishMentionedPostResult,
  RegisterRelayNameRequest,
  RelayNameClaim,
  ResolvedMention,
  SendMessageResult,
} from '../types';
import type { CommunityInfo } from '../types/boards';
import { HarborError } from '../utils/errors';
import type { Comment, CommentCount } from './comments';
import type { RssFeedConfig, WallPreviewPerspective, WallVisibilityStats } from './feed';

type Command<Args, Result> = { args: Args; result: Result };
type NoArgs<Result> = Command<undefined, Result>;

interface RawLikeSummary {
  postId?: string;
  post_id?: string;
  totalLikes?: number;
  total_likes?: number;
  userHasLiked?: boolean;
  user_has_liked?: boolean;
}

export interface LinkPreviewData {
  url: string;
  title: string | null;
  description: string | null;
  image_url: string | null;
  site_name: string | null;
}

interface HarborCommandMap {
  list_accounts: NoArgs<AccountInfo[]>;
  get_account: Command<{ accountId: string }, AccountInfo | null>;
  get_active_account: NoArgs<AccountInfo | null>;
  has_accounts: NoArgs<boolean>;
  set_active_account: Command<{ accountId: string }, AccountInfo>;
  remove_account: Command<{ accountId: string; deleteData: boolean }, void>;
  update_account_metadata: Command<
    {
      accountId: string;
      displayName?: string;
      bio?: string | null;
      avatarHash?: string | null;
    },
    AccountInfo
  >;
  export_identity_backup: Command<{ path: string; password: string }, IdentityBackupExportResult>;
  restore_identity_backup: Command<{ path: string; password: string }, IdentityBackupRestoreResult>;
  delete_account_profile: Command<
    { accountId: string; password: string },
    DeleteAccountProfileResult
  >;

  get_communities: NoArgs<CommunityInfo[]>;
  join_community: Command<{ relayAddress: string }, void>;
  leave_community: Command<{ relayPeerId: string }, void>;
  get_boards: Command<{ relayPeerId: string }, BoardInfo[]>;
  get_board_posts: Command<
    { relayPeerId: string; boardId: string; limit?: number; beforeTimestamp?: number },
    BoardPost[]
  >;
  submit_board_post: Command<{ relayPeerId: string; boardId: string; contentText: string }, void>;
  delete_board_post: Command<{ relayPeerId: string; postId: string }, void>;
  sync_board: Command<{ relayPeerId: string; boardId: string }, void>;

  get_active_calls: NoArgs<CallSession[]>;
  get_call_history: Command<{ limit: number }, CallSession[]>;
  get_active_group_calls: NoArgs<GroupCallRoom[]>;
  send_group_membership: Command<
    {
      input: {
        roomId?: string;
        creatorPeerId?: string;
        action: GroupMembershipAction;
        rosterVersion: number;
        participants: string[];
        mediaMode: 'audio' | 'video';
      };
    },
    GroupMembershipSignal
  >;
  start_call: Command<{ calleePeerId: string; sdp: string }, OfferResult>;
  answer_call: Command<{ callId: string; callerPeerId: string; sdp: string }, AnswerResult>;
  send_ice_candidate: Command<
    {
      callId: string;
      targetPeerId: string;
      candidate: string;
      sdpMid?: string;
      sdpMlineIndex?: number;
    },
    IceResult
  >;
  hangup_call: Command<
    { callId: string; targetPeerId: string; reason?: HangupReason },
    HangupResult
  >;
  decline_call: Command<{ callId: string; callerPeerId: string }, HangupResult>;
  busy_call: Command<{ callId: string; callerPeerId: string }, HangupResult>;
  process_offer: Command<
    {
      callId: string;
      callerPeerId: string;
      calleePeerId: string;
      sdp: string;
      timestamp: number;
      signature: number[];
    },
    void
  >;
  process_answer: HarborCommandMap['process_offer'];
  process_ice_candidate: Command<
    {
      callId: string;
      senderPeerId: string;
      candidate: string;
      sdpMid?: string;
      sdpMlineIndex?: number;
      timestamp: number;
      signature: number[];
    },
    void
  >;
  process_hangup: Command<
    {
      callId: string;
      senderPeerId: string;
      reason: string;
      timestamp: number;
      signature: number[];
    },
    void
  >;

  add_comment: Command<{ postId: string; content: string }, Comment>;
  get_comments: Command<{ postId: string }, Comment[]>;
  delete_comment: Command<{ commentId: string }, boolean>;
  get_comment_counts: Command<{ postIds: string[] }, CommentCount[]>;

  get_contacts: NoArgs<Contact[]>;
  get_active_contacts: NoArgs<Contact[]>;
  get_contact: Command<{ peerId: string }, Contact | null>;
  add_contact: Command<
    {
      peerId: string;
      publicKey: number[];
      x25519Public: number[];
      displayName: string;
      avatarHash: string | null;
      bio: string | null;
    },
    number
  >;
  block_contact: Command<{ peerId: string }, boolean>;
  unblock_contact: Command<{ peerId: string }, boolean>;
  remove_contact: Command<{ peerId: string }, boolean>;
  is_contact: Command<{ peerId: string }, boolean>;
  is_contact_blocked: Command<{ peerId: string }, boolean>;
  request_peer_identity: Command<{ peerId: string }, string>;
  get_contact_requests: NoArgs<ContactRequest[]>;
  respond_contact_request: Command<
    { requestId: string; decision: 'accepted' | 'declined' },
    ContactDecisionResult
  >;
  retry_contact_request: Command<{ requestId: string }, void>;

  get_feed: Command<{ limit?: number; beforeTimestamp?: number }, FeedItem[]>;
  get_wall: Command<{ authorPeerId: string; limit?: number; beforeTimestamp?: number }, FeedItem[]>;
  sync_feed_from_relay: NoArgs<void>;
  sync_wall_to_relay: NoArgs<void>;
  fetch_contact_wall_from_relay: Command<{ authorPeerId: string }, void>;
  sync_wall_social_events_to_relay: NoArgs<number>;
  fetch_wall_social_events_from_relay: Command<{ authorPeerId: string; postIds: string[] }, void>;
  get_wall_preview: Command<
    { perspective: WallPreviewPerspective; limit?: number; beforeTimestamp?: number },
    FeedItem[]
  >;
  get_wall_visibility_stats: NoArgs<WallVisibilityStats>;
  generate_rss_feed: Command<{ config?: RssFeedConfig }, string>;
  get_rss_feed_url: NoArgs<string>;

  get_identity_initialization_state: NoArgs<IdentityInitializationResult>;
  has_identity: NoArgs<boolean>;
  is_identity_unlocked: NoArgs<boolean>;
  get_identity_info: NoArgs<IdentityInfo | null>;
  create_identity: Command<{ request: CreateIdentityRequest }, IdentityInfo>;
  unlock_identity: Command<{ passphrase: string }, IdentityInfo>;
  change_identity_password: Command<{ currentPassword: string; newPassword: string }, void>;
  lock_identity: NoArgs<void>;
  update_display_name: Command<{ displayName: string }, void>;
  update_bio: Command<{ bio: string | null }, void>;
  update_profile_avatar: Command<{ filePath: string | null }, IdentityInfo>;
  update_passphrase_hint: Command<{ hint: string | null }, void>;
  get_peer_id: NoArgs<string>;
  register_relay_name: Command<{ request: RegisterRelayNameRequest }, RelayNameClaim>;
  get_local_name_claim: NoArgs<RelayNameClaim | null>;
  verify_name_claim: Command<{ claim: RelayNameClaim }, boolean>;
  get_identity_entry_state: NoArgs<{
    mode: 'required' | 'unverified' | 'verified';
    claim: RelayNameClaim | null;
  }>;
  get_identity_publishing_state: NoArgs<{
    mode: 'required' | 'unverified' | 'verified';
  }>;
  set_identity_publishing_mode: Command<{ mode: 'unverified' | 'verified' }, void>;

  like_post: Command<{ postId: string }, RawLikeSummary>;
  unlike_post: Command<{ postId: string }, RawLikeSummary>;
  get_post_likes: Command<{ postId: string }, RawLikeSummary>;
  get_posts_likes_batch: Command<{ postIds: string[] }, RawLikeSummary[]>;
  get_my_liked_posts: NoArgs<string[]>;

  export_logs: NoArgs<string>;
  get_log_path: NoArgs<string>;
  cleanup_logs: Command<{ maxFiles: number }, void>;
  save_to_downloads: Command<{ filename: string; content: string }, string>;
  fetch_link_preview: Command<{ url: string }, LinkPreviewData>;

  store_media: Command<{ filePath: string; mimeType?: string }, StoredMediaInfo>;
  get_media_asset: Command<{ hash: string }, MediaAssetInfo>;
  has_media: Command<{ hash: string }, boolean>;
  preload_missing_media: NoArgs<number>;
  ensure_media_transfer: Command<{ input: EnsureMediaTransferInput }, MediaTransferState>;
  get_media_transfer: Command<{ mediaHash: string }, MediaTransferState | null>;
  retry_media_transfer: Command<{ mediaHash: string }, MediaTransferState>;
  get_media_cache_diagnostics: NoArgs<MediaCacheDiagnostics>;
  update_media_cache_settings: Command<{ settings: MediaCacheSettings }, MediaCacheDiagnostics>;

  resolve_private_mention: Command<{ qualifiedName: string }, ResolvedMention>;
  create_post_with_mentions: Command<
    { request: PublishMentionedPostRequest },
    PublishMentionedPostResult
  >;
  list_pending_mentions: NoArgs<MentionReceipt[]>;
  review_private_mention: Command<
    {
      mentionId: string;
      decision: 'accept-notification' | 'accept-repost' | 'decline' | 'block';
    },
    void
  >;

  send_message: Command<
    {
      peerId: string;
      content: string;
      contentType?: string;
      replyTo?: string;
    },
    SendMessageResult
  >;
  get_messages: Command<{ peerId: string; limit?: number; beforeTimestamp?: number }, Message[]>;
  get_conversations: NoArgs<Conversation[]>;
  mark_conversation_read: Command<{ peerId: string }, number>;
  get_messaging_privacy_policy: NoArgs<{ readReceiptsEnabled: boolean }>;
  set_read_receipts_enabled: Command<{ enabled: boolean }, { readReceiptsEnabled: boolean }>;
  get_unread_count: Command<{ peerId: string }, number>;
  get_total_unread_count: NoArgs<number>;
  clear_conversation_history: Command<{ peerId: string }, void>;
  delete_conversation: Command<{ peerId: string }, void>;
  edit_message: Command<{ messageId: string; newContent: string; peerId: string }, void>;

  start_network: Command<{ enableMdns?: boolean; bootstrapNodes?: string[] }, void>;
  stop_network: NoArgs<void>;
  is_network_running: NoArgs<boolean>;
  get_connected_peers: NoArgs<PeerInfo[]>;
  get_network_stats: NoArgs<NetworkStats>;
  bootstrap_network: NoArgs<void>;
  get_listening_addresses: NoArgs<string[]>;
  connect_to_peer: Command<{ multiaddr: string }, void>;
  add_bootstrap_node: Command<{ multiaddr: string }, void>;
  add_relay_server: Command<{ multiaddr: string }, void>;
  connect_to_public_relays: NoArgs<void>;
  get_nat_status: NoArgs<string>;
  get_shareable_addresses: NoArgs<string[]>;
  get_shareable_contact_string: NoArgs<string>;
  add_contact_from_string: Command<{ contactString: string }, AddContactResult>;
  sync_feed: Command<{ limit?: number }, void>;

  grant_permission: Command<
    { subjectPeerId: string; capability: Capability; expiresInSeconds?: number | null },
    GrantResult
  >;
  revoke_permission: Command<{ grantId: string }, boolean>;
  peer_has_capability: Command<{ peerId: string; capability: Capability }, boolean>;
  we_have_capability: Command<{ issuerPeerId: string; capability: Capability }, boolean>;
  get_granted_permissions: NoArgs<PermissionInfo[]>;
  get_received_permissions: NoArgs<PermissionInfo[]>;
  get_chat_peers: NoArgs<string[]>;
  grant_all_permissions: Command<{ subjectPeerId: string }, GrantResult[]>;

  create_post: Command<
    {
      contentType: string;
      contentText?: string;
      visibility?: PostVisibility;
      media?: CreatePostMediaInput[];
    },
    CreatePostResult
  >;
  update_post: Command<{ postId: string; contentText?: string }, PostMutationResult>;
  delete_post: Command<{ postId: string }, PostMutationResult>;
  get_post: Command<{ postId: string }, Post | null>;
  get_my_posts: Command<{ limit?: number; beforeTimestamp?: number }, Post[]>;
  get_posts_by_author: Command<
    { authorPeerId: string; limit?: number; beforeTimestamp?: number },
    Post[]
  >;
  add_post_media: Command<
    {
      params: {
        postId: string;
        mediaHash: string;
        mediaType: string;
        mimeType: string;
        fileName: string;
        fileSize: number;
        width?: number;
        height?: number;
        durationSeconds?: number;
        sortOrder?: number;
      };
    },
    void
  >;
  get_post_media: Command<{ postId: string }, PostMedia[]>;
}

export type CommandName = keyof HarborCommandMap;
export type CommandArgs<Name extends CommandName> = HarborCommandMap[Name]['args'];
export type CommandResult<Name extends CommandName> = HarborCommandMap[Name]['result'];

type CommandParameters<Name extends CommandName> =
  CommandArgs<Name> extends undefined ? [] : [args: CommandArgs<Name>];

/**
 * The only frontend boundary for invoking Harbor's Rust command API.
 *
 * Command names, arguments, and results are checked as one contract, and every
 * rejected value is converted into a stable HarborError before it reaches a
 * store or component.
 */
export async function invokeCommand<Name extends CommandName>(
  command: Name,
  ...parameters: CommandParameters<Name>
): Promise<CommandResult<Name>> {
  try {
    const args = parameters[0];
    return args === undefined
      ? await invoke<CommandResult<Name>>(command)
      : await invoke<CommandResult<Name>>(command, args);
  } catch (error) {
    throw HarborError.fromUnknown(error, { command });
  }
}
