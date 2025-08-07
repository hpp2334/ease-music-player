package com.kutedev.easemusicplayer.components

import EaseImage
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import com.kutedev.easemusicplayer.R
import uniffi.ease_client_backend.DataSourceKey

@Composable
fun MusicCover(
    modifier: Modifier,
    coverDataSourceKey: DataSourceKey?
) {
    Box(
        modifier = modifier,
    ) {
        if (coverDataSourceKey == null) {
            Image(
                modifier = Modifier.fillMaxSize(),
                painter = painterResource(id = R.drawable.cover_default_image), // Replace with actual image resource
                contentDescription = null,
            )
        } else {
            EaseImage(
                modifier = Modifier.background(MaterialTheme.colorScheme.onSurfaceVariant).fillMaxSize(),
                dataSourceKey = coverDataSourceKey,
                contentScale = ContentScale.FillWidth,
            )
        }
    }
}