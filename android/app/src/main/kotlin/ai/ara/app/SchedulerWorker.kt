package ai.ara.app

import android.content.Context
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import java.util.concurrent.TimeUnit

/**
 * Checks every 15 minutes (Android's own `PeriodicWorkRequest` floor, which
 * `ara_scheduler` itself enforces as its repeat-interval minimum too) for
 * scheduled tasks that are due, turns each into a real mission, and hands
 * it to [MissionWorker]. The scheduler only ever decides *when* a mission
 * gets created — everything about running it (budget, retries, approval
 * gating, crash-safe persistence) is the exact same path as a mission the
 * user started by hand.
 */
class SchedulerWorker(appContext: Context, params: WorkerParameters) :
    CoroutineWorker(appContext, params) {

    override suspend fun doWork(): Result {
        val bridge = CoreBridge.get(applicationContext)
        val created = try {
            bridge.io { fireDueScheduledTasks() }
        } catch (e: Exception) {
            e.rethrowIfCancellation()
            return Result.retry()
        }
        created.forEach { id -> MissionWorker.enqueueNext(applicationContext, id) }
        return Result.success()
    }

    companion object {
        private const val UNIQUE_NAME = "ara-scheduler-tick"

        /** Idempotent: safe to call on every app start (`KEEP` policy). */
        fun ensureScheduled(context: Context) {
            val request = PeriodicWorkRequestBuilder<SchedulerWorker>(15, TimeUnit.MINUTES)
                .setConstraints(Constraints.Builder().setRequiresBatteryNotLow(true).build())
                .build()
            WorkManager.getInstance(context)
                .enqueueUniquePeriodicWork(UNIQUE_NAME, ExistingPeriodicWorkPolicy.KEEP, request)
        }
    }
}
