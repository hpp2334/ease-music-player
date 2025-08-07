package com.kutedev.easemusicplayer.components

import EaseImage
import android.provider.CalendarContract.Colors
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.R
import uniffi.ease_client_backend.DataSourceKey

@Composable
fun ImportCover(
    dataSourceKey: DataSourceKey?,
    onAdd: () -> Unit,
    onRemove: () -> Unit,
) {
    if (dataSourceKey != null) {
        Box(
            modifier = Modifier
                .size(90.dp)
        ) {
            Box(
                modifier = Modifier
                    .offset(0.dp, 10.dp)
                    .clip(RoundedCornerShape(6.dp))
                    .width(80.dp)
                    .height(80.dp)
            ) {
                EaseImage(
                    dataSourceKey = dataSourceKey,
                    modifier = Modifier.fillMaxSize(),
                    contentScale = ContentScale.FillWidth
                )
            }
            Box(
                modifier = Modifier
                    .offset(70.dp)
                    .clip(RoundedCornerShape(999.dp))
                    .clickable {
                        onRemove()
                    }
                    .background(MaterialTheme.colorScheme.error)
                    .width(20.dp)
                    .height(20.dp),
                contentAlignment = Alignment.Center
            ) {
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(999.dp))
                        .background(Color.White)
                        .width(8.dp)
                        .height(2.dp)
                )
            }
        }
    } else {
        Box(
            modifier = Modifier
                .height(86.dp)
        ) {
            Box(
                modifier = Modifier
                    .offset(0.dp, 10.dp)
                    .clip(RoundedCornerShape(6.dp))
                    .clickable {
                        onAdd()
                    }
                    .background(MaterialTheme.colorScheme.surfaceVariant)
                    .width(80.dp)
                    .height(80.dp),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    modifier = Modifier
                        .size(20.dp),
                    painter = painterResource(R.drawable.icon_plus),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}