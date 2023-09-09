import 'package:ease_music_player/bridge_generated.dart';
import 'package:ease_music_player/global.dart';
import 'package:ease_music_player/routes/music/mini_player.dart';
import 'package:ease_music_player/widgets/icons.dart';
import 'package:flutter/material.dart';

import 'clip_shadow.dart';

EaseIconsToken getNavigationBarIconToken(RootRouteSubKey screenKey) {
  switch (screenKey) {
    case RootRouteSubKey.Playlist:
      return EaseIconsTokens.album;
    case RootRouteSubKey.Dashboard:
      return EaseIconsTokens.dashboard;
    case RootRouteSubKey.Setting:
      return EaseIconsTokens.setting;
  }
}

class _NavigationBarItem extends StatelessWidget {
  final RootRouteSubKey screenKey;
  final bool isActive;
  final void Function() onTap;

  const _NavigationBarItem({
    required this.screenKey,
    required this.isActive,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return Material(
      color: EaseColors.surface,
      child: InkWell(
        onTap: () {
          if (!isActive) {
            onTap();
          }
        },
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 40, vertical: 18),
          child: EaseIcon(
            size: 20,
            color: isActive ? EaseColors.primary : EaseColors.disabled,
            iconToken: getNavigationBarIconToken(screenKey),
          ),
        ),
      ),
    );
  }
}

class NavigationBarClipper extends CustomClipper<Path> {
  @override
  Path getClip(Size size) {
    const targetHeight = 20.0;
    const targetWidth = 50.0;

    final path = Path();
    path.moveTo(0, targetHeight);
    path.quadraticBezierTo(0, 0, targetWidth, 0);
    path.lineTo(size.width - targetWidth, 0);
    path.quadraticBezierTo(size.width, 0, size.width, targetHeight);
    path.lineTo(size.width, size.height);
    path.lineTo(0, size.height);
    path.lineTo(0, 0);
    return path;
  }

  @override
  bool shouldReclip(covariant CustomClipper<Path> oldClipper) {
    return true;
  }
}

class _NavigationBar extends StatelessWidget {
  final RootRouteSubKey activeSubScreenKey;
  final void Function(RootRouteSubKey)? onTapNav;

  const _NavigationBar({
    required this.activeSubScreenKey,
    required this.onTapNav,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      color: EaseColors.surface,
      child: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          _NavigationBarItem(
            screenKey: RootRouteSubKey.Playlist,
            isActive: activeSubScreenKey == RootRouteSubKey.Playlist,
            onTap: () {
              if (onTapNav != null) {
                onTapNav!(RootRouteSubKey.Playlist);
              }
            },
          ),
          _NavigationBarItem(
            screenKey: RootRouteSubKey.Dashboard,
            isActive: activeSubScreenKey == RootRouteSubKey.Dashboard,
            onTap: () {
              if (onTapNav != null) {
                onTapNav!(RootRouteSubKey.Dashboard);
              }
            },
          ),
          _NavigationBarItem(
            screenKey: RootRouteSubKey.Setting,
            isActive: activeSubScreenKey == RootRouteSubKey.Setting,
            onTap: () {
              if (onTapNav != null) {
                onTapNav!(RootRouteSubKey.Setting);
              }
            },
          ),
        ],
      ),
    );
  }
}

class EaseScreenContainer extends StatelessWidget {
  final Widget child;
  final PreferredSizeWidget? header;
  final RootRouteSubKey? subScreenKey;
  final Color? barColor;
  final bool? hidePlayer;
  final void Function(RootRouteSubKey)? onTapNav;

  const EaseScreenContainer({
    super.key,
    required this.child,
    this.header,
    this.subScreenKey,
    this.barColor,
    this.hidePlayer,
    this.onTapNav,
  });

  @override
  Widget build(BuildContext context) {
    final viewTopHeight = MediaQuery.of(context).viewPadding.top;
    final showBottomBar = subScreenKey != null || hidePlayer != true;

    final body = Column(
      children: [
        Expanded(child: child),
        if (showBottomBar)
          ClipShadow(
            clipper: NavigationBarClipper(),
            boxShadows: [
              BoxShadow(
                color: Colors.black.withAlpha(25),
                blurRadius: 8,
              ),
            ],
            child: Column(children: [
              if (hidePlayer != true) const MiniPlayerWidget(),
              if (subScreenKey != null)
                _NavigationBar(
                  activeSubScreenKey: subScreenKey!,
                  onTapNav: onTapNav,
                ),
            ]),
          ),
      ],
    );

    return Scaffold(
      body: Container(
        padding: EdgeInsets.only(top: viewTopHeight),
        color: EaseColors.surface,
        child: header == null
            ? body
            : Stack(
                children: [
                  Positioned(
                    top: 0,
                    left: 0,
                    right: 0,
                    height: header!.preferredSize.height,
                    child: header!,
                  ),
                  Positioned(
                    top: header!.preferredSize.height,
                    bottom: 0,
                    left: 0,
                    right: 0,
                    child: body,
                  ),
                ],
              ),
      ),
    );
  }
}
