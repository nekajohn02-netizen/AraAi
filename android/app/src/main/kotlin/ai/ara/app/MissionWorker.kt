package ai.ara.app

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import org.json.JSONObject
import uniffi.ara_ffi.CancelHandle
import uniffi.ara_ffi.FfiException
import java.util.concurrent.TimeUnit

/**
 * Runs one mission step per execution, then re-enqueues itself if the
 * mission is still running — so a mission keeps making progress across
 * hours even if the app is closed, without ever running continuously in
 * the background (each step is one bounded model call, not a busy loop).
 *
 * The core (not this worker) decides what "done" or "needs approval"
 * means; this class only schedules and notifies.
 */
class MissionWorker(appContext: Context, params: WorkerParameters) :
    CoroutineWorker(appContext, params) {

    override suspend fun doWork(): Result {
        val id = inputData.getString(KEY_MISSION_ID) ?: return Result.failure()
        val bridge = CoreBridge.get(applicationContext)
        val current = runCatching { bridge.io { JSONObject(mission(id)) } }.getOrNull()
            ?: return Result.failure()
        if (current.getString("status") != "running") return Result.success()

        val handle = CancelHandle()
        val m = try {
            bridge.io { JSONObject(runMissionStep(id, handle)) }
        } catch (e: FfiException) {
            notify(applicationContext, id, current.getString("goal"), e.message ?: "")
            return Result.failure()
        } finally {
            if (isStopped) handle.cancel()
        }

        val goal = m.getString("goal")
        return when {
            m.getJSONArray("pending").length() > 0 -> {
                notify(applicationContext, id, goal, applicationContext.getString(R.string.mission_needs_approval))
                Result.success()
            }
            m.getString("status") == "completed" -> {
                notify(applicationContext, id, goal, applicationContext.getString(R.string.mission_done))
                Result.success()
            }
            m.getString("status") == "running" -> {
                enqueueNext(applicationContext, id)
                Result.success()
            }
            // paused (incl. cancelled-while-generating), cancelled, failed,
            // budget_exhausted: nothing more to schedule.
            else -> Result.success()
        }
    }

    companion object {
        const val KEY_MISSION_ID = "mission_id"
        private const val CHANNEL_ID = "ara_missions"
        private const val STEP_DELAY_SECONDS = 5L

        /** Schedules (or replaces) the next step for a running mission. */
        fun enqueueNext(context: Context, id: String) {
            val request = OneTimeWorkRequestBuilder<MissionWorker>()
                .setInputData(workDataOf(KEY_MISSION_ID to id))
                .setInitialDelay(STEP_DELAY_SECONDS, TimeUnit.SECONDS)
                .setConstraints(Constraints.Builder().setRequiresBatteryNotLow(true).build())
                .build()
            WorkManager.getInstance(context)
                .enqueueUniqueWork("mission-$id", ExistingWorkPolicy.REPLACE, request)
        }

        fun cancel(context: Context, id: String) {
            WorkManager.getInstance(context).cancelUniqueWork("mission-$id")
        }

        private fun ensureChannel(context: Context) {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
            val mgr = context.getSystemService(NotificationManager::class.java) ?: return
            if (mgr.getNotificationChannel(CHANNEL_ID) != null) return
            mgr.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_ID,
                    context.getString(R.string.mission_channel),
                    NotificationManager.IMPORTANCE_DEFAULT,
                ),
            )
        }

        private fun notify(context: Context, id: String, goal: String, text: String) {
            ensureChannel(context)
            val openApp = Intent(context, MainActivity::class.java)
                .setFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
            val pending = PendingIntent.getActivity(
                context, id.hashCode(), openApp,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
            val notification = NotificationCompat.Builder(context, CHANNEL_ID)
                .setSmallIcon(android.R.drawable.ic_menu_agenda)
                .setContentTitle(goal.take(80))
                .setContentText(text)
                .setAutoCancel(true)
                .setContentIntent(pending)
                .build()
            val canNotify = Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
                context.checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) ==
                    PackageManager.PERMISSION_GRANTED
            if (canNotify) {
                runCatching { NotificationManagerCompat.from(context).notify(id.hashCode(), notification) }
            }
        }
    }
}
