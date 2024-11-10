package com.kutedev.easemusicplayer.components

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import coil3.compose.AsyncImage
import com.kutedev.easemusicplayer.R

@Composable
fun MusicCover(
    modifier: Modifier,
    coverUrl: String,
) {
    Box(
        modifier = modifier,
    ) {
        if (coverUrl.isEmpty()) {
            Image(
                modifier = Modifier.fillMaxSize(),
                painter = painterResource(id = R.drawable.cover_default_image), // Replace with actual image resource
                contentDescription = null,
            )
        } else {
            AsyncImage(
                modifier = Modifier.background(MaterialTheme.colorScheme.onSurfaceVariant).fillMaxSize(),
                model = coverUrl,
                contentDescription = null,
                contentScale = ContentScale.FillWidth,
            )
        }
    }
}