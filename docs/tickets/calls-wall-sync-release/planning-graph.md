---
ldgr_doc: 1
kind: graph
id: graph.calls-wall-sync-release.v1
schema: ldgr.graph.v1
status: ready
tags:
- harbor
- dependency-graph
---

# Calls, Video, and Wall Sync Dependency Graph

This graph references ticket projection artifacts recorded in the parent Harbor LDGR run.

```ldgr-graph yaml
nodes:
- id: ticket.0001-reconcile-release-capability-contract
  artifact: artifact:7
  work_item: work:0001-reconcile-release-capability-contract
- id: ticket.0101-calling-signaling-transport
  artifact: artifact:8
  work_item: work:0101-calling-signaling-transport
- id: ticket.0102-call-session-state-history
  artifact: artifact:9
  work_item: work:0102-call-session-state-history
- id: ticket.0103-webrtc-audio-runtime
  artifact: artifact:10
  work_item: work:0103-webrtc-audio-runtime
- id: ticket.0104-calling-ui-and-events
  artifact: artifact:11
  work_item: work:0104-calling-ui-and-events
- id: ticket.0105-call-ice-nat-configuration
  artifact: artifact:12
  work_item: work:0105-call-ice-nat-configuration
- id: ticket.0106-voice-call-integration-validation
  artifact: artifact:13
  work_item: work:0106-voice-call-integration-validation
- id: ticket.0201-group-call-topology-contract
  artifact: artifact:14
  work_item: work:0201-group-call-topology-contract
- id: ticket.0202-video-call-media-runtime
  artifact: artifact:15
  work_item: work:0202-video-call-media-runtime
- id: ticket.0203-group-call-signaling-membership
  artifact: artifact:16
  work_item: work:0203-group-call-signaling-membership
- id: ticket.0204-group-call-media-layout-runtime
  artifact: artifact:17
  work_item: work:0204-group-call-media-layout-runtime
- id: ticket.0205-video-group-call-validation
  artifact: artifact:18
  work_item: work:0205-video-group-call-validation
- id: ticket.0301-wall-author-visibility-settings
  artifact: artifact:19
  work_item: work:0301-wall-author-visibility-settings
- id: ticket.0302-wall-media-signature-integrity
  artifact: artifact:20
  work_item: work:0302-wall-media-signature-integrity
- id: ticket.0303-wall-edit-delete-sync
  artifact: artifact:21
  work_item: work:0303-wall-edit-delete-sync
- id: ticket.0304-wall-author-social-ui
  artifact: artifact:22
  work_item: work:0304-wall-author-social-ui
- id: ticket.0305-wall-preview-rss-share-ui
  artifact: artifact:23
  work_item: work:0305-wall-preview-rss-share-ui
- id: ticket.0401-contact-wall-view
  artifact: artifact:24
  work_item: work:0401-contact-wall-view
- id: ticket.0402-feed-interactions-real
  artifact: artifact:25
  work_item: work:0402-feed-interactions-real
- id: ticket.0403-consumer-comments-reactions-ui
  artifact: artifact:26
  work_item: work:0403-consumer-comments-reactions-ui
- id: ticket.0404-consumer-media-fetch-lifecycle
  artifact: artifact:27
  work_item: work:0404-consumer-media-fetch-lifecycle
- id: ticket.0501-relay-wall-permission-enforcement
  artifact: artifact:28
  work_item: work:0501-relay-wall-permission-enforcement
- id: ticket.0502-wall-sync-cursors-pagination
  artifact: artifact:29
  work_item: work:0502-wall-sync-cursors-pagination
- id: ticket.0503-wall-event-reconciliation-tombstones
  artifact: artifact:30
  work_item: work:0503-wall-event-reconciliation-tombstones
- id: ticket.0504-wall-social-event-model
  artifact: artifact:31
  work_item: work:0504-wall-social-event-model
- id: ticket.0505-wall-sync-status-observability
  artifact: artifact:32
  work_item: work:0505-wall-sync-status-observability
- id: ticket.0506-wall-sync-multi-profile-validation
  artifact: artifact:33
  work_item: work:0506-wall-sync-multi-profile-validation
- id: ticket.0601-docs-and-release-gates-for-calls-wall-sync
  artifact: artifact:34
  work_item: work:0601-docs-and-release-gates-for-calls-wall-sync
edges:
- dependency: ticket.0001-reconcile-release-capability-contract
  dependent: ticket.0101-calling-signaling-transport
- dependency: ticket.0001-reconcile-release-capability-contract
  dependent: ticket.0301-wall-author-visibility-settings
- dependency: ticket.0001-reconcile-release-capability-contract
  dependent: ticket.0501-relay-wall-permission-enforcement
- dependency: ticket.0101-calling-signaling-transport
  dependent: ticket.0102-call-session-state-history
- dependency: ticket.0101-calling-signaling-transport
  dependent: ticket.0103-webrtc-audio-runtime
- dependency: ticket.0102-call-session-state-history
  dependent: ticket.0103-webrtc-audio-runtime
- dependency: ticket.0103-webrtc-audio-runtime
  dependent: ticket.0104-calling-ui-and-events
- dependency: ticket.0105-call-ice-nat-configuration
  dependent: ticket.0103-webrtc-audio-runtime
- dependency: ticket.0104-calling-ui-and-events
  dependent: ticket.0106-voice-call-integration-validation
- dependency: ticket.0105-call-ice-nat-configuration
  dependent: ticket.0106-voice-call-integration-validation
- dependency: ticket.0201-group-call-topology-contract
  dependent: ticket.0203-group-call-signaling-membership
- dependency: ticket.0201-group-call-topology-contract
  dependent: ticket.0204-group-call-media-layout-runtime
- dependency: ticket.0103-webrtc-audio-runtime
  dependent: ticket.0202-video-call-media-runtime
- dependency: ticket.0105-call-ice-nat-configuration
  dependent: ticket.0202-video-call-media-runtime
- dependency: ticket.0202-video-call-media-runtime
  dependent: ticket.0204-group-call-media-layout-runtime
- dependency: ticket.0203-group-call-signaling-membership
  dependent: ticket.0204-group-call-media-layout-runtime
- dependency: ticket.0204-group-call-media-layout-runtime
  dependent: ticket.0205-video-group-call-validation
- dependency: ticket.0301-wall-author-visibility-settings
  dependent: ticket.0305-wall-preview-rss-share-ui
- dependency: ticket.0302-wall-media-signature-integrity
  dependent: ticket.0404-consumer-media-fetch-lifecycle
- dependency: ticket.0503-wall-event-reconciliation-tombstones
  dependent: ticket.0303-wall-edit-delete-sync
- dependency: ticket.0504-wall-social-event-model
  dependent: ticket.0304-wall-author-social-ui
- dependency: ticket.0504-wall-social-event-model
  dependent: ticket.0402-feed-interactions-real
- dependency: ticket.0504-wall-social-event-model
  dependent: ticket.0403-consumer-comments-reactions-ui
- dependency: ticket.0501-relay-wall-permission-enforcement
  dependent: ticket.0401-contact-wall-view
- dependency: ticket.0502-wall-sync-cursors-pagination
  dependent: ticket.0401-contact-wall-view
- dependency: ticket.0501-relay-wall-permission-enforcement
  dependent: ticket.0502-wall-sync-cursors-pagination
- dependency: ticket.0503-wall-event-reconciliation-tombstones
  dependent: ticket.0506-wall-sync-multi-profile-validation
- dependency: ticket.0504-wall-social-event-model
  dependent: ticket.0506-wall-sync-multi-profile-validation
- dependency: ticket.0505-wall-sync-status-observability
  dependent: ticket.0506-wall-sync-multi-profile-validation
- dependency: ticket.0106-voice-call-integration-validation
  dependent: ticket.0601-docs-and-release-gates-for-calls-wall-sync
- dependency: ticket.0205-video-group-call-validation
  dependent: ticket.0601-docs-and-release-gates-for-calls-wall-sync
- dependency: ticket.0506-wall-sync-multi-profile-validation
  dependent: ticket.0601-docs-and-release-gates-for-calls-wall-sync
```
