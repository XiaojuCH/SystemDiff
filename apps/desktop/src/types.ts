export type Locale = "en-US" | "zh-CN";

export type SessionStage =
  | "ready"
  | "starting"
  | "capturing"
  | "finishing"
  | "results";

export interface DesktopSessionState {
  stage: SessionStage;
  presentation: DesktopPresentation | null;
  cleanup_pending: boolean;
}

export interface DesktopPresentation {
  contract_version: 1;
  started_at_utc: string;
  finished_at_utc: string;
  summary: DesktopSummary;
  groups: DesktopChangeGroup[];
  coverage_notices: DesktopCoverageNotice[];
}

export interface DesktopSummary {
  confirmed_change_count: number;
  inconclusive_change_count: number;
}

export type DesktopGroupId = "startup" | "windows_services" | "other";
export type DesktopChangeKind =
  | "added"
  | "removed"
  | "modified"
  | "unchanged"
  | "inconclusive";

export interface DesktopChangeGroup {
  group_id: DesktopGroupId;
  heading_message_id: string;
  empty_message_id: string;
  items: DesktopChangeItem[];
}

export interface DesktopChangeItem {
  change: DesktopChangeKind;
  message_id: string;
  headline: DesktopValue;
  fields: DesktopField[];
}

export type DesktopField =
  | { field_id: string; mode: "current"; value: DesktopValue }
  | { field_id: string; mode: "changed"; before: DesktopValue; after: DesktopValue };

export type DesktopValue =
  | { kind: "evidence"; value: string }
  | { kind: "message"; message_id: string }
  | { kind: "number"; value: number }
  | { kind: "boolean"; value: boolean }
  | { kind: "evidence_list"; values: string[] };

export interface DesktopCoverageNotice {
  group_id: DesktopGroupId | null;
  message_id: string;
  scope_message_id: string;
  before_status: DesktopCoverageStatus;
  after_status: DesktopCoverageStatus;
}

export type DesktopCoverageStatus =
  | "complete"
  | "partial"
  | "permission_denied"
  | "unavailable"
  | "unsupported"
  | "failed"
  | "not_present";

export interface DesktopCommandError {
  code: string;
  message_id: string;
  technical_details: string;
}
