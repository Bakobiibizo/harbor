/**
 * =============================================================================
 * MOCK DATA BOUNDARY DOCUMENTATION
 * =============================================================================
 *
 * STATUS: FULLY DEPRECATED / DEAD CODE
 *
 * This file contains mock peer data, conversations, auto-reply logic, and
 * user post management that was used during early UI development before the
 * real Tauri/Rust backend was connected. As of the current codebase, this
 * store is NOT imported or used by any page, component, or other store.
 * It is only re-exported from `src/stores/index.ts` for backwards
 * compatibility but has zero consumers.
 *
 * --- Feature-by-feature data source summary ---
 *
 * CHAT (src/pages/Chat.tsx)
 *   Data source: REAL BACKEND
 *   Stores used: useContactsStore, useMessagingStore
 *   Details: Conversations are built from real contacts loaded via
 *     contactsService (Tauri invoke). Messages are loaded/sent via
 *     useMessagingStore which calls Tauri commands (get_conversations,
 *     get_messages, send_message). No mock data involved.
 *
 * FEED (src/pages/Feed.tsx)
 *   Data source: REAL BACKEND
 *   Stores used: useFeedStore, useContactsStore
 *   Details: Feed items are loaded via feedService.getFeed() which calls
 *     the Tauri backend. Refresh triggers networkService.syncFeed() to
 *     pull posts from connected peers, then reloads from local DB.
 *     No mock data involved.
 *
 * WALL / JOURNAL (src/pages/Wall.tsx)
 *   Data source: REAL BACKEND
 *   Stores used: useIdentityStore, useWallStore
 *   Details: Posts are loaded/created/updated/deleted via postsService
 *     which calls Tauri commands. Likes are local-only (toggled in
 *     Zustand state, not persisted to backend). No mock data involved.
 *
 * NETWORK (src/pages/Network.tsx)
 *   Data source: REAL BACKEND
 *   Stores used: useIdentityStore, useNetworkStore, useContactsStore,
 *     useSettingsStore
 *   Details: All peer discovery, connection, relay management, and
 *     contact management use real libp2p networking via Tauri commands
 *     through networkService and contactsService. No mock data involved.
 *
 * SETTINGS (src/pages/Settings.tsx)
 *   Data source: REAL BACKEND + localStorage
 *   Stores used: useIdentityStore, useSettingsStore
 *   Details: Profile data comes from the real identity service (Tauri).
 *     Preferences (theme, notifications, privacy toggles) are persisted
 *     via Zustand persist middleware to localStorage. No mock data involved.
 *
 * BOARDS (src/pages/Boards.tsx)
 *   Data source: REAL BACKEND
 *   Stores used: useBoardsStore, useIdentityStore
 *   Details: Communities, boards, and posts are all loaded via
 *     boardsService which calls Tauri commands. No mock data involved.
 *
 * --- What this file originally provided (now unused) ---
 *
 * - 6 mock peers (Alice, Bob, Carol, David, Eva, Frank) with bios,
 *   online status, avatar gradients, and wall posts
 * - Mock conversations with pre-seeded messages
 * - Auto-reply system for simulating online peer responses
 * - User's own wall posts (demo data)
 * - Feed aggregation from mock peer walls
 * - Saved posts, hidden posts, snoozed users, archived conversations
 *
 * --- Migration / cleanup TODO ---
 *
 * - This entire file can be safely deleted once the barrel export in
 *   src/stores/index.ts is also updated to remove the re-export.
 * - The types (MockPeer, MockPost, MockConversation, etc.) exported from
 *   src/stores/index.ts are also unused and can be removed.
 * - No pages or components import from this file.
 * - The only remaining reference outside this file is in
 *   src/stores/network.test.ts, which defines its own local `mockPeers`
 *   variable (unrelated to this store).
 *
 * =============================================================================
 */

import { create } from 'zustand';

// Types for mock peer data
export interface MockPost {
  id: string;
  content: string;
  timestamp: Date;
  likes: number;
  comments: number;
  media?: { type: 'image' | 'video'; url: string }[];
}

export interface MockPeer {
  id: string;
  peerId: string;
  name: string;
  bio: string;
  online: boolean;
  avatarGradient: string;
  wall: MockPost[];
}

export interface MockMessage {
  id: string;
  content: string;
  timestamp: Date;
  isMine: boolean;
}

export interface MockConversation {
  id: string;
  peerId: string;
  name: string;
  online: boolean;
  avatarGradient: string;
  lastMessage: string;
  timestamp: Date;
  unread: number;
  messages: MockMessage[];
}

// Auto-reply responses based on keywords
const autoReplies: { keywords: string[]; responses: string[] }[] = [
  {
    keywords: ['hello', 'hi', 'hey', 'sup'],
    responses: ["Hey there! How's it going?", 'Hi! Great to hear from you!', "Hello! What's up?"],
  },
  {
    keywords: ['how are you', "how's it going", "what's up"],
    responses: [
      "I'm doing great, thanks for asking! How about you?",
      'Pretty good! Just working on some projects. You?',
      "Can't complain! What's new with you?",
    ],
  },
  {
    keywords: ['harbor', 'app', 'project'],
    responses: [
      'Harbor is such a cool project! The P2P architecture is really impressive.',
      'I love how Harbor keeps everything decentralized. No data harvesting!',
      'The encryption in Harbor makes me feel so much safer about my messages.',
    ],
  },
  {
    keywords: ['p2p', 'peer', 'network', 'libp2p'],
    responses: [
      'Peer-to-peer is the future! Centralized services are so 2010s.',
      "I've been learning a lot about libp2p lately. It's fascinating stuff!",
      'The NAT traversal in P2P apps is getting really good these days.',
    ],
  },
  {
    keywords: ['thanks', 'thank you', 'appreciate'],
    responses: [
      "You're welcome! Happy to help!",
      'No problem at all!',
      "Anytime! That's what friends are for.",
    ],
  },
  {
    keywords: ['bye', 'goodbye', 'see you', 'later'],
    responses: ['See you later! Take care!', 'Goodbye! Chat soon!', 'Later! Have a great day!'],
  },
  {
    keywords: ['?'],
    responses: [
      "That's a good question! Let me think about it...",
      "Hmm, I'm not entirely sure. What do you think?",
      "Interesting question! I'd need to look into that more.",
    ],
  },
];

// Default responses when no keywords match
const defaultResponses = [
  "That's interesting! Tell me more.",
  'I see what you mean.',
  'Yeah, I totally agree with that.',
  'That makes sense!',
  'Oh nice! Sounds cool.',
  'Haha, yeah!',
  'I was just thinking the same thing!',
  "For sure! That's a great point.",
];

// Generate a response based on the incoming message
function generateAutoReply(message: string): string {
  const lowerMessage = message.toLowerCase();

  // Check for keyword matches
  for (const rule of autoReplies) {
    if (rule.keywords.some((keyword) => lowerMessage.includes(keyword))) {
      return rule.responses[Math.floor(Math.random() * rule.responses.length)];
    }
  }

  // Return a default response
  return defaultResponses[Math.floor(Math.random() * defaultResponses.length)];
}

// Avatar gradients for visual variety
const avatarGradients = [
  'linear-gradient(135deg, hsl(220 91% 54%), hsl(262 83% 58%))',
  'linear-gradient(135deg, hsl(262 83% 58%), hsl(330 81% 60%))',
  'linear-gradient(135deg, hsl(152 69% 40%), hsl(180 70% 45%))',
  'linear-gradient(135deg, hsl(36 90% 55%), hsl(15 80% 55%))',
  'linear-gradient(135deg, hsl(200 80% 50%), hsl(220 91% 54%))',
  'linear-gradient(135deg, hsl(340 75% 55%), hsl(10 80% 60%))',
];

// Create mock peers with their own walls
const mockPeers: MockPeer[] = [
  {
    id: 'peer-alice',
    peerId: '12D3KooWAbCdEfGhIjKlMnOpQrStUvWxYz',
    name: 'Alice Chen',
    bio: 'Full-stack developer passionate about decentralized systems. Building the future of communication.',
    online: true,
    avatarGradient: avatarGradients[0],
    wall: [
      {
        id: 'alice-1',
        content:
          'Just deployed my first smart contract on Ethereum! The gas fees were brutal but the satisfaction of seeing it work was worth it. Next step: optimizing for L2 solutions. 🚀',
        timestamp: new Date(Date.now() - 1000 * 60 * 30),
        likes: 24,
        comments: 8,
      },
      {
        id: 'alice-2',
        content:
          "Hot take: The best code is the code you don't write. Every line is a liability. Simplicity wins every time.",
        timestamp: new Date(Date.now() - 1000 * 60 * 60 * 4),
        likes: 45,
        comments: 12,
      },
      {
        id: 'alice-3',
        content:
          "Reading 'The Network State' by Balaji. Fascinating ideas about how digital communities can evolve into something more. What are your thoughts on decentralized governance?",
        timestamp: new Date(Date.now() - 1000 * 60 * 60 * 12),
        likes: 18,
        comments: 5,
      },
    ],
  },
  {
    id: 'peer-bob',
    peerId: '12D3KooWXyZaBcDeFgHiJkLmNoPqRsTuVw',
    name: 'Bob Wilson',
    bio: 'Systems engineer. Rust enthusiast. Making computers go brrr.',
    online: true,
    avatarGradient: avatarGradients[1],
    wall: [
      {
        id: 'bob-1',
        content:
          'Just hit 1 million messages processed per second on our new message queue. The secret? Zero-copy buffers and careful memory alignment. Performance matters!',
        timestamp: new Date(Date.now() - 1000 * 60 * 45),
        likes: 67,
        comments: 21,
      },
      {
        id: 'bob-2',
        content:
          "Excited to announce that I'm joining the Harbor project as a contributor! Looking forward to building the future of decentralized communication together.",
        timestamp: new Date(Date.now() - 1000 * 60 * 60 * 8),
        likes: 89,
        comments: 34,
      },
      {
        id: 'bob-3',
        content:
          "Debugging tip: When you're stuck, explain the problem to a rubber duck. If that doesn't work, take a walk. The solution often comes when you stop forcing it.",
        timestamp: new Date(Date.now() - 1000 * 60 * 60 * 24),
        likes: 156,
        comments: 28,
      },
    ],
  },
  {
    id: 'peer-carol',
    peerId: '12D3KooWQrStUvWxYzAbCdEfGhIjKlMnOp',
    name: 'Carol Davis',
    bio: 'UX designer turned developer. Making tech accessible and beautiful.',
    online: false,
    avatarGradient: avatarGradients[2],
    wall: [
      {
        id: 'carol-1',
        content:
          'Design tip: The best interface is no interface. Before adding a new screen or dialog, ask yourself: can the system figure this out automatically?',
        timestamp: new Date(Date.now() - 1000 * 60 * 60 * 2),
        likes: 78,
        comments: 15,
      },
      {
        id: 'carol-2',
        content:
          'Tip for fellow developers: When working with libp2p, make sure to handle peer disconnections gracefully. The network is inherently unstable, and your app needs to handle that elegantly.',
        timestamp: new Date(Date.now() - 1000 * 60 * 60 * 12),
        likes: 34,
        comments: 9,
      },
    ],
  },
  {
    id: 'peer-david',
    peerId: '12D3KooWMnOpQrStUvWxYzAbCdEfGhIjKl',
    name: 'David Miller',
    bio: 'Privacy advocate. Building tools for a more private internet.',
    online: true,
    avatarGradient: avatarGradients[3],
    wall: [
      {
        id: 'david-1',
        content:
          "The more I use peer-to-peer apps, the more I realize how much we've given up to centralized platforms. Privacy isn't just a feature—it's a right.",
        timestamp: new Date(Date.now() - 1000 * 60 * 60),
        likes: 123,
        comments: 45,
      },
      {
        id: 'david-2',
        content:
          "Just finished setting up my own email server. Yes, it's a pain. Yes, deliverability is a nightmare. But knowing my emails aren't being scanned? Priceless.",
        timestamp: new Date(Date.now() - 1000 * 60 * 60 * 18),
        likes: 89,
        comments: 32,
      },
      {
        id: 'david-3',
        content:
          "Reminder: Your data is valuable. If a service is free, you're the product. That's why I'm so excited about Harbor - no servers, no data harvesting, just direct communication.",
        timestamp: new Date(Date.now() - 1000 * 60 * 60 * 36),
        likes: 201,
        comments: 67,
      },
    ],
  },
  {
    id: 'peer-eva',
    peerId: '12D3KooWEfGhIjKlMnOpQrStUvWxYzAbCd',
    name: 'Eva Martinez',
    bio: 'Cryptography researcher. Turning math into privacy.',
    online: true,
    avatarGradient: avatarGradients[4],
    wall: [
      {
        id: 'eva-1',
        content:
          "New paper just dropped: 'Post-Quantum Key Exchange for Real-Time Communication'. We show that lattice-based crypto can be practical for P2P messaging. Link in bio!",
        timestamp: new Date(Date.now() - 1000 * 60 * 90),
        likes: 156,
        comments: 38,
      },
      {
        id: 'eva-2',
        content:
          'People ask why I work on cryptography. Simple: Math is the only thing that can truly protect your secrets. Governments change, companies fail, but prime numbers are forever.',
        timestamp: new Date(Date.now() - 1000 * 60 * 60 * 6),
        likes: 234,
        comments: 52,
      },
    ],
  },
  {
    id: 'peer-frank',
    peerId: '12D3KooWKlMnOpQrStUvWxYzAbCdEfGhIj',
    name: 'Frank Johnson',
    bio: 'DevOps engineer by day, open source contributor by night.',
    online: false,
    avatarGradient: avatarGradients[5],
    wall: [
      {
        id: 'frank-1',
        content:
          'Finally automated our entire deployment pipeline. From commit to production in under 5 minutes with full rollback capability. CI/CD done right feels magical.',
        timestamp: new Date(Date.now() - 1000 * 60 * 60 * 3),
        likes: 98,
        comments: 24,
      },
      {
        id: 'frank-2',
        content:
          'Unpopular opinion: Kubernetes is overkill for 90% of projects. Sometimes a simple VPS with Docker Compose is all you need. Fight me.',
        timestamp: new Date(Date.now() - 1000 * 60 * 60 * 28),
        likes: 312,
        comments: 89,
      },
    ],
  },
];

// Create initial conversations from mock peers
const initialConversations: MockConversation[] = mockPeers.map((peer, index) => ({
  id: `conv-${peer.id}`,
  peerId: peer.peerId,
  name: peer.name,
  online: peer.online,
  avatarGradient: peer.avatarGradient,
  lastMessage:
    index === 0
      ? 'Hey! Are you coming to the meetup?'
      : index === 1
        ? 'Thanks for the help yesterday!'
        : index === 2
          ? "I'll send over the design files soon"
          : index === 3
            ? 'Privacy is non-negotiable!'
            : index === 4
              ? 'Check out my new paper!'
              : "Let's sync up on the deployment",
  timestamp: new Date(Date.now() - 1000 * 60 * (5 + index * 30)),
  unread: index === 0 ? 2 : index === 3 ? 1 : 0,
  messages: [
    {
      id: `${peer.id}-msg-1`,
      content: `Hey! Nice to connect with you on Harbor!`,
      timestamp: new Date(Date.now() - 1000 * 60 * 60 * 2),
      isMine: false,
    },
    {
      id: `${peer.id}-msg-2`,
      content: 'Thanks! Great to be here. How are you finding the app?',
      timestamp: new Date(Date.now() - 1000 * 60 * 60 * 1.5),
      isMine: true,
    },
    {
      id: `${peer.id}-msg-3`,
      content: "It's amazing! The P2P connectivity is really smooth.",
      timestamp: new Date(Date.now() - 1000 * 60 * 60),
      isMine: false,
    },
  ],
}));

// User's own posts interface
export interface UserPost {
  id: string;
  content: string;
  timestamp: Date;
  likes: number;
  comments: number;
  liked: boolean; // Has the user liked their own post
  media?: { type: 'image' | 'video'; url: string; name?: string }[];
}

// Saved posts interface
export interface SavedPost {
  postId: string;
  peerId: string;
  savedAt: Date;
}

export interface HiddenPost {
  postId: string;
  peerId: string;
  hiddenAt: Date;
}

export interface SnoozedUser {
  peerId: string;
  snoozedAt: Date;
  snoozedUntil: Date; // When snooze expires
}

export interface ArchivedConversation {
  conversationId: string;
  archivedAt: Date;
}

// Zustand store interface
interface MockPeersState {
  peers: MockPeer[];
  conversations: MockConversation[];
  archivedConversations: ArchivedConversation[];
  userPosts: UserPost[];
  savedPosts: SavedPost[];
  hiddenPosts: HiddenPost[];
  snoozedUsers: SnoozedUser[];
  likedPosts: Set<string>; // Track which feed posts user has liked (format: "peerId:postId")

  // Actions
  sendMessage: (conversationId: string, content: string) => void;
  likePost: (peerId: string, postId: string) => void;
  getAllFeedPosts: () => Array<
    MockPost & {
      author: Pick<MockPeer, 'id' | 'name' | 'avatarGradient' | 'peerId'>;
      likedByUser: boolean;
    }
  >;

  // User posts actions
  addUserPost: (
    content: string,
    media?: { type: 'image' | 'video'; url: string; name?: string }[],
  ) => void;
  likeUserPost: (postId: string) => void;
  deleteUserPost: (postId: string) => void;

  // Saved posts actions
  toggleSavePost: (peerId: string, postId: string) => void;
  isPostSaved: (peerId: string, postId: string) => boolean;
  getSavedPosts: () => Array<
    MockPost & {
      author: Pick<MockPeer, 'id' | 'name' | 'avatarGradient' | 'peerId'>;
      likedByUser: boolean;
      savedAt: Date;
    }
  >;

  // Hidden posts actions
  hidePost: (peerId: string, postId: string) => void;
  unhidePost: (peerId: string, postId: string) => void;
  isPostHidden: (peerId: string, postId: string) => boolean;

  // Snoozed users actions
  snoozeUser: (peerId: string, durationHours: number) => void;
  unsnoozeUser: (peerId: string) => void;
  isUserSnoozed: (peerId: string) => boolean;

  // Conversation management actions
  archiveConversation: (conversationId: string) => void;
  unarchiveConversation: (conversationId: string) => void;
  isConversationArchived: (conversationId: string) => boolean;
  clearConversationHistory: (conversationId: string) => void;
  deleteConversation: (conversationId: string) => void;
  getActiveConversations: () => MockConversation[];
  getArchivedConversationsList: () => MockConversation[];
}

// Initial user posts (demo data)
const initialUserPosts: UserPost[] = [
  {
    id: '1',
    content:
      "Just launched Harbor - a decentralized P2P chat application! It's been an incredible journey building this. Check out the features: end-to-end encryption, local-first data, and peer-to-peer communication. No central servers, no data harvesting. Your identity stays with you.",
    timestamp: new Date(Date.now() - 1000 * 60 * 60 * 2),
    likes: 12,
    comments: 3,
    liked: false,
  },
  {
    id: '2',
    content:
      'The beauty of decentralized systems is that you own your data. No company can access your messages, no algorithm decides what you see. Just direct, secure communication with the people you choose.',
    timestamp: new Date(Date.now() - 1000 * 60 * 60 * 24),
    likes: 8,
    comments: 2,
    liked: false,
  },
  {
    id: '3',
    content:
      'Working on voice calling next! WebRTC signaling through libp2p is going to be interesting. Stay tuned for updates.',
    timestamp: new Date(Date.now() - 1000 * 60 * 60 * 48),
    likes: 15,
    comments: 5,
    liked: false,
  },
];

export const useMockPeersStore = create<MockPeersState>((set, get) => ({
  peers: mockPeers,
  conversations: initialConversations,
  archivedConversations: [],
  userPosts: initialUserPosts,
  savedPosts: [],
  hiddenPosts: [],
  snoozedUsers: [],
  likedPosts: new Set<string>(),

  sendMessage: (conversationId: string, content: string) => {
    const timestamp = new Date();
    const myMessage: MockMessage = {
      id: `msg-${Date.now()}`,
      content,
      timestamp,
      isMine: true,
    };

    set((state) => ({
      conversations: state.conversations.map((conv) =>
        conv.id === conversationId
          ? {
              ...conv,
              messages: [...conv.messages, myMessage],
              lastMessage: content,
              timestamp,
              unread: 0,
            }
          : conv,
      ),
    }));

    // Simulate auto-reply after a delay (1-3 seconds)
    const conversation = get().conversations.find((c) => c.id === conversationId);
    if (conversation?.online) {
      const delay = 1000 + Math.random() * 2000;
      setTimeout(() => {
        const replyContent = generateAutoReply(content);
        const replyTimestamp = new Date();
        const replyMessage: MockMessage = {
          id: `msg-${Date.now()}-reply`,
          content: replyContent,
          timestamp: replyTimestamp,
          isMine: false,
        };

        set((state) => ({
          conversations: state.conversations.map((conv) =>
            conv.id === conversationId
              ? {
                  ...conv,
                  messages: [...conv.messages, replyMessage],
                  lastMessage: replyContent,
                  timestamp: replyTimestamp,
                }
              : conv,
          ),
        }));
      }, delay);
    }
  },

  likePost: (peerId: string, postId: string) => {
    const likeKey = `${peerId}:${postId}`;
    const { likedPosts } = get();

    // If already liked, unlike it
    if (likedPosts.has(likeKey)) {
      const newLikedPosts = new Set(likedPosts);
      newLikedPosts.delete(likeKey);
      set((state) => ({
        likedPosts: newLikedPosts,
        peers: state.peers.map((peer) =>
          peer.peerId === peerId
            ? {
                ...peer,
                wall: peer.wall.map((post) =>
                  post.id === postId ? { ...post, likes: Math.max(0, post.likes - 1) } : post,
                ),
              }
            : peer,
        ),
      }));
    } else {
      // Like the post
      const newLikedPosts = new Set(likedPosts);
      newLikedPosts.add(likeKey);
      set((state) => ({
        likedPosts: newLikedPosts,
        peers: state.peers.map((peer) =>
          peer.peerId === peerId
            ? {
                ...peer,
                wall: peer.wall.map((post) =>
                  post.id === postId ? { ...post, likes: post.likes + 1 } : post,
                ),
              }
            : peer,
        ),
      }));
    }
  },

  getAllFeedPosts: () => {
    const { peers, likedPosts } = get();

    // Collect all posts from all peers with author info
    const allPosts = peers.flatMap((peer) =>
      peer.wall.map((post) => ({
        ...post,
        author: {
          id: peer.id,
          name: peer.name,
          avatarGradient: peer.avatarGradient,
          peerId: peer.peerId,
        },
        likedByUser: likedPosts.has(`${peer.peerId}:${post.id}`),
      })),
    );

    // Sort by timestamp (most recent first)
    return allPosts.sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime());
  },

  // User posts actions
  addUserPost: (
    content: string,
    media?: { type: 'image' | 'video'; url: string; name?: string }[],
  ) => {
    const newPost: UserPost = {
      id: Date.now().toString(),
      content,
      timestamp: new Date(),
      likes: 0,
      comments: 0,
      liked: false,
      media,
    };
    set((state) => ({
      userPosts: [newPost, ...state.userPosts],
    }));
  },

  likeUserPost: (postId: string) => {
    set((state) => ({
      userPosts: state.userPosts.map((post) =>
        post.id === postId
          ? {
              ...post,
              liked: !post.liked,
              likes: post.liked ? post.likes - 1 : post.likes + 1,
            }
          : post,
      ),
    }));
  },

  deleteUserPost: (postId: string) => {
    set((state) => ({
      userPosts: state.userPosts.filter((post) => post.id !== postId),
    }));
  },

  // Saved posts actions
  toggleSavePost: (peerId: string, postId: string) => {
    set((state) => {
      const existingIndex = state.savedPosts.findIndex(
        (s) => s.peerId === peerId && s.postId === postId,
      );
      if (existingIndex >= 0) {
        return {
          savedPosts: state.savedPosts.filter((_, i) => i !== existingIndex),
        };
      }
      return {
        savedPosts: [...state.savedPosts, { peerId, postId, savedAt: new Date() }],
      };
    });
  },

  isPostSaved: (peerId: string, postId: string) => {
    return get().savedPosts.some((s) => s.peerId === peerId && s.postId === postId);
  },

  getSavedPosts: () => {
    const { savedPosts, peers, likedPosts } = get();

    return savedPosts
      .map((saved) => {
        // Find the peer who authored the post
        const peer = peers.find((p) => p.peerId === saved.peerId);
        if (!peer) return null;

        // Find the post in the peer's wall
        const post = peer.wall.find((p) => p.id === saved.postId);
        if (!post) return null;

        const likeKey = `${peer.peerId}:${post.id}`;

        return {
          ...post,
          author: {
            id: peer.id,
            name: peer.name,
            avatarGradient: peer.avatarGradient,
            peerId: peer.peerId,
          },
          likedByUser: likedPosts.has(likeKey),
          savedAt: saved.savedAt,
        };
      })
      .filter((p): p is NonNullable<typeof p> => p !== null)
      .sort((a, b) => b.savedAt.getTime() - a.savedAt.getTime());
  },

  // Hidden posts actions
  hidePost: (peerId: string, postId: string) => {
    set((state) => ({
      hiddenPosts: [...state.hiddenPosts, { peerId, postId, hiddenAt: new Date() }],
    }));
  },

  unhidePost: (peerId: string, postId: string) => {
    set((state) => ({
      hiddenPosts: state.hiddenPosts.filter((h) => !(h.peerId === peerId && h.postId === postId)),
    }));
  },

  isPostHidden: (peerId: string, postId: string) => {
    return get().hiddenPosts.some((h) => h.peerId === peerId && h.postId === postId);
  },

  // Snoozed users actions
  snoozeUser: (peerId: string, durationHours: number) => {
    const now = new Date();
    const snoozedUntil = new Date(now.getTime() + durationHours * 60 * 60 * 1000);
    set((state) => ({
      snoozedUsers: [
        ...state.snoozedUsers.filter((s) => s.peerId !== peerId),
        { peerId, snoozedAt: now, snoozedUntil },
      ],
    }));
  },

  unsnoozeUser: (peerId: string) => {
    set((state) => ({
      snoozedUsers: state.snoozedUsers.filter((s) => s.peerId !== peerId),
    }));
  },

  isUserSnoozed: (peerId: string) => {
    const snooze = get().snoozedUsers.find((s) => s.peerId === peerId);
    if (!snooze) return false;
    // Check if snooze has expired
    return new Date() < snooze.snoozedUntil;
  },

  // Conversation management actions
  archiveConversation: (conversationId: string) => {
    set((state) => ({
      archivedConversations: [
        ...state.archivedConversations,
        { conversationId, archivedAt: new Date() },
      ],
    }));
  },

  unarchiveConversation: (conversationId: string) => {
    set((state) => ({
      archivedConversations: state.archivedConversations.filter(
        (a) => a.conversationId !== conversationId,
      ),
    }));
  },

  isConversationArchived: (conversationId: string) => {
    return get().archivedConversations.some((a) => a.conversationId === conversationId);
  },

  clearConversationHistory: (conversationId: string) => {
    set((state) => ({
      conversations: state.conversations.map((conv) =>
        conv.id === conversationId
          ? { ...conv, messages: [], lastMessage: 'No messages yet', unread: 0 }
          : conv,
      ),
    }));
  },

  deleteConversation: (conversationId: string) => {
    set((state) => ({
      conversations: state.conversations.filter((c) => c.id !== conversationId),
      archivedConversations: state.archivedConversations.filter(
        (a) => a.conversationId !== conversationId,
      ),
    }));
  },

  getActiveConversations: () => {
    const { conversations, archivedConversations } = get();
    const archivedIds = new Set(archivedConversations.map((a) => a.conversationId));
    return conversations.filter((c) => !archivedIds.has(c.id));
  },

  getArchivedConversationsList: () => {
    const { conversations, archivedConversations } = get();
    const archivedIds = new Set(archivedConversations.map((a) => a.conversationId));
    return conversations.filter((c) => archivedIds.has(c.id));
  },
}));
