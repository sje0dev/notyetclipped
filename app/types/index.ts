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
  status: string
  last_error: string | null
  next_check_at: string | null
  created_at: string
  recording_id: string | null
  buffer_seconds: number
  audio_db: number | null
  started_at: string | null
}

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
  stats: { recording: number; streamers: number; clips_today: number; clips_ready: number; stored_bytes: number; pending: number; failed: number }
  streamers: Streamer[]
  clips: Clip[]
  activity: { id: number; kind: string; message: string; created_at: string }[]
  settings: Settings
}
