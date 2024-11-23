package com.kutedev.easemusicplayer.widgets

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.navigation.NavHostController
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.kutedev.easemusicplayer.core.UIBridgeController
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import uniffi.ease_client.RoutesKey

private val LocalNavController = compositionLocalOf<NavHostController> {
    error("No LocalNavController provided")
}

//@Composable
//fun getCurrentRoute(): Flow<String?> {
//    return LocalNavController.current.currentBackStackEntryFlow
//        .map { currentRoute -> currentRoute.destination.route }
//}

@Composable
fun RoutesProvider(
    block: @Composable (navHostController: NavHostController) -> Unit
) {
    val bridge = UIBridgeController.current
    CompositionLocalProvider(LocalNavController provides rememberNavController()) {
        val controller = LocalNavController.current
        bridge.routerInternal.install(controller)

        block(controller)
    }
}