package hu.autotherm.autocrm.ui.orders

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import hu.autotherm.autocrm.data.api.StageEntry
import hu.autotherm.autocrm.ui.theme.Done
import hu.autotherm.autocrm.ui.theme.Steel200
import hu.autotherm.autocrm.ui.theme.Steel500
import hu.autotherm.autocrm.ui.theme.Steel900
import hu.autotherm.autocrm.util.formatDateTime

/**
 * The web client's StageRail (frontend/src/components/ui/StageRail.tsx), drawn over the
 * stage HISTORY the backend returns oldest-first (repo/stages.rs ORDER BY entered_at, id).
 * The last entry is the current stage: steel dot, semibold label, "{days} napja itt".
 * Earlier entries are done: green check. Same Hungarian strings as src/messages/hu.json.
 */
@Composable
fun StageRail(
    history: List<StageEntry>,
    daysInStage: Long,
    openBlockers: Int,
    modifier: Modifier = Modifier,
) {
    if (history.isEmpty()) {
        Text("Nincs fáziselőzmény.", style = MaterialTheme.typography.bodyLarge, color = Steel500, modifier = modifier)
        return
    }
    Column(modifier) {
        history.forEachIndexed { i, entry ->
            val current = i == history.lastIndex
            // IntrinsicSize.Min bounds the dot column to the text height: without it
            // fillMaxHeight measures against unbounded LazyColumn constraints and the
            // connector collapses (or fills the viewport).
            Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min)) {
                Box(Modifier.width(19.dp).fillMaxHeight()) {
                    if (!current) {
                        Box(
                            Modifier.align(Alignment.TopCenter).padding(top = 20.dp)
                                .width(1.dp).fillMaxHeight()
                                .background(Steel200),
                        )
                    }
                    Box(Modifier.size(19.dp).align(Alignment.TopCenter)) {
                        if (current) {
                            Box(
                                Modifier.size(10.dp).align(Alignment.Center)
                                    .background(Steel900, CircleShape),
                            )
                        } else {
                            Icon(
                                Icons.Filled.CheckCircle,
                                contentDescription = "Kész",
                                tint = Done,
                                modifier = Modifier.size(19.dp),
                            )
                        }
                    }
                }
                Column(Modifier.padding(start = 12.dp, bottom = if (current) 0.dp else 20.dp)) {
                    Text(
                        entry.labelHu,
                        style = MaterialTheme.typography.bodyLarge.copy(
                            fontWeight = if (current) FontWeight.SemiBold else FontWeight.Medium,
                        ),
                        color = Steel900,
                    )
                    if (current) {
                        val blockers = if (openBlockers > 0) " · $openBlockers nyitott akadály" else ""
                        Text(
                            "$daysInStage napja itt$blockers",
                            style = MaterialTheme.typography.labelMedium,
                            color = Steel500,
                        )
                    }
                    Text(
                        listOfNotNull(formatDateTime(entry.enteredAt), entry.enteredByName)
                            .joinToString(" · "),
                        style = MaterialTheme.typography.labelMedium,
                        color = Steel500,
                    )
                    entry.note?.let {
                        Text(it, style = MaterialTheme.typography.bodyLarge, color = Steel900)
                    }
                }
            }
        }
    }
}
