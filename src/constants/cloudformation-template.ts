// Keep the templates shown in the app identical to the reviewed deployment files.
// Vite's raw imports bundle the YAML as copyable text without duplicating secrets or
// allowing the UI and operator documentation to drift apart.
import relayTemplate from '../../infrastructure/relay-cloudformation.yaml?raw';
import communityRelayTemplate from '../../infrastructure/community-relay-cloudformation.yaml?raw';

export const RELAY_CLOUDFORMATION_TEMPLATE = relayTemplate;
export const COMMUNITY_RELAY_CLOUDFORMATION_TEMPLATE = communityRelayTemplate;
