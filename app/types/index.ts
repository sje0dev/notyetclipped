export interface Streamer {
  id: string
  login: string
  enabled: boolean
  quality: string
  priority: number
  pre_roll: number
  post_roll: number
  threshold_db: number
  auto_clip: boolean
  trigger_profile: TriggerProfile
  trigger_state: { signal?: TriggerSignal; updated_at?: string } | null
  status: string
  last_error: string | null
  next_check_at: string | null
  created_at: string
  recording_id: string | null
  buffer_seconds: number
  audio_db: number | null
  started_at: string | null
}

export type TriggerMetric = 'audio_rise' | 'audio_level' | 'chat_messages' | 'chat_users' | 'chat_rate'
export interface TriggerCondition { metric: TriggerMetric; op: 'gte' | 'lte'; value: number }
export interface TriggerRule { name: string; enabled: boolean; hold_seconds: number; conditions: TriggerCondition[] }
export interface ContentRule { id: string; name: string; enabled: boolean; kind: 'story' | 'visual'; description: string }
export interface SemanticTrigger { enabled: boolean; stories: boolean; visual: boolean; interval_seconds: number; min_confidence: number; guidance: string; exclusions: string; context_seconds: number; language: string; use_feedback: boolean; rules: ContentRule[] }
export interface TriggerProfile { version: number; warmup_seconds: number; cooldown_seconds: number; hourly_limit: number; rules: TriggerRule[]; semantic: SemanticTrigger }
export interface AiSettings { enabled: boolean; model: string; hourly_limit: number; daily_limit: number; cpu_threads: number; frame_count: number; paced: boolean }
export interface AiRun { id: string; streamer_id: string; status: string; message: string; created_at: string; clip_id: string | null; preparation_ms: number | null; inference_ms: number | null; result: { title: string; reason: string; confidence: number; kind: string; trigger_id: string } | null }
export interface AiStatus {
  settings: AiSettings; paused_reason: string | null; requests_hour: number; requests_day: number; transcription_ready: boolean
  auth: { connected: boolean; plan_allowed: boolean; active: string | null; accounts: { id: string; email: string }[] }; runs: AiRun[]
  scheduler: { active_streamers: number; budget_interval_seconds: number | null; preparation_seconds: number | null; inference_seconds: number | null }
}
export interface TriggerSignal {
  time_ms: number
  audio_level: number
  audio_rise: number
  baseline_db: number
  calibrated_seconds: number
  chat_messages: number | null
  chat_users: number | null
  chat_rate: number | null
}
export interface TriggerSignals {
  recording: { id: string; started_at: string; ended_at: string | null; trigger_state: { signal?: TriggerSignal; updated_at?: string } } | null
  frames: TriggerSignal[]
  hourly_count: number
  last_trigger_at: string | null
}
export interface SignalEvent extends TriggerSignals { type: 'signals.updated'; id: string; audio_db: number | null; buffer_seconds: number }
export interface MetricSummary { mean: number; low: number; high: number; count: number }
export interface TriggerTest { hits: { time_ms: number; rule: string; reason: string }[]; seconds: number; recording_started_at: string | null; chat_available: boolean; sample_count: number }

export interface Clip {
  id: string
  streamer_id: string
  streamer_login: string
  title: string
  status: string
  trigger: string
  score: number
  reason: string
  size_bytes: number
  duration: number
  favorite: boolean
  rating: number
  error: string | null
  created_at: string
  ready_at: string | null
  attempts: number
  thumbnail_url: string | null
  video_url?: string | null
  download_url?: string | null
}

export interface CropRect { x: number; y: number; width: number; height: number }
export interface EditingSettings { auto_queue: boolean; framing: 'fit' | 'fill'; gameplay: CropRect; facecam_enabled: boolean; facecam: CropRect; facecam_position: 'top' | 'bottom'; facecam_percent: number; chat_mode: 'keep' | 'hide' | 'panel'; chat: CropRect }
export interface EditJob { id: string; clip_id: string; title: string; streamer_id: string; streamer_login: string; status: string; profile: EditingSettings; size_bytes: number; error: string | null; created_at: string; video_url?: string; download_url?: string }

export interface Settings {
  max_recordings: number
  max_exports: number
  buffer_seconds: number
  scratch_budget_gb: number
  min_free_disk_gb: number
  offline_poll_seconds: number
  retention_days: number
}

export interface Health {
  database: boolean
  storage: boolean
  worker: boolean
  bucket: string
  worker_details: {
    active_recordings?: number
    active_exports?: number
    scratch_bytes?: number
    disk_free_bytes?: number
    paused_reason?: string | null
  }
}

export interface Dashboard {
  trend: { day: string; automatic: number; manual: number }[]
  stats: { recording: number; streamers: number; clips_today: number; clips_ready: number; stored_bytes: number; pending: number; failed: number }
  streamers: Streamer[]
  clips: Clip[]
  activity: { id: number; kind: string; message: string; created_at: string }[]
  settings: Settings
}
