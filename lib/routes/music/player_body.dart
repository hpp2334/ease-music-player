import 'dart:math';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:ease_music_player/global.dart';
import 'package:ease_music_player/models/current_music.dart';
import 'package:ease_music_player/models/current_music_lyric.dart';
import 'package:ease_music_player/services/player.service.dart';
import 'package:ease_music_player/services/resource.service.dart';
import 'package:ease_music_player/services/router.service.dart';
import 'package:ease_music_player/widgets/icons.dart';
import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:provider/provider.dart';

enum _DraggingAnimateTo {
  previous,
  next,
  origin,
}

class PlayerBody extends StatefulWidget {
  final double containerWidth;
  final double screenWidth;
  final double screenHeight;

  const PlayerBody({
    super.key,
    required this.containerWidth,
    required this.screenWidth,
    required this.screenHeight,
  });

  @override
  State<PlayerBody> createState() => _PlayerBodyState();
}

class _PlayerBodyState extends State<PlayerBody> with TickerProviderStateMixin {
  bool isCover = true;
  double? startDraggingOffsetX;
  double currentDraggingOffsetX = 0;
  AnimationController? draggingEndAnimationController;
  static const dragChangeMusicVelocityThreshold = 500;
  static const scrollAnimationDuration = 300;

  @override
  void initState() {
    super.initState();
  }

  @override
  void dispose() {
    clearAnimation();
    super.dispose();
  }

  double getDragChangeMusicLimit() {
    return widget.screenWidth;
  }

  void clearAnimation() {
    if (draggingEndAnimationController != null) {
      draggingEndAnimationController!.dispose();
    }
    draggingEndAnimationController = null;
  }

  void draggingAnimateTo(_DraggingAnimateTo to) {
    clearAnimation();
    double offsetX = 0;
    if (to == _DraggingAnimateTo.next) {
      offsetX = -getDragChangeMusicLimit();
    } else if (to == _DraggingAnimateTo.previous) {
      offsetX = getDragChangeMusicLimit();
    }

    draggingEndAnimationController = AnimationController(
      duration: const Duration(milliseconds: scrollAnimationDuration),
      vsync: this,
    );
    final draggingEndAnimation =
        Tween(begin: currentDraggingOffsetX, end: offsetX)
            .chain(CurveTween(curve: Curves.easeOut))
            .animate(draggingEndAnimationController!);
    draggingEndAnimationController!.addListener(() {
      currentDraggingOffsetX = draggingEndAnimation.value;
      setState(() {});
    });
    draggingEndAnimationController!.addStatusListener((status) {
      if (status != AnimationStatus.completed) {
        return;
      }

      clearAnimation();
      setState(() {
        startDraggingOffsetX = null;
        currentDraggingOffsetX = 0;
      });

      if (to == _DraggingAnimateTo.next) {
        playerService.skipToNext();
        setState(() {
          isCover = true;
        });
      } else if (to == _DraggingAnimateTo.previous) {
        playerService.skipToPrevious();
        setState(() {
          isCover = true;
        });
      }
    });
    draggingEndAnimationController!.forward();
  }

  @override
  Widget build(BuildContext context) {
    const coverPadding = EdgeInsets.only(
      top: 0,
      bottom: 22,
      left: 22,
      right: 22,
    );
    final dragChangeMusicLimit = getDragChangeMusicLimit();
    final dragChangeMusicThreshold = dragChangeMusicLimit * 0.6;

    final state = context.watch<CurrentMusicModel>().value;

    final hasPrevious = state.canPlayPrevious;
    final hasNext = state.canPlayNext;

    return GestureDetector(
      onHorizontalDragStart: (e) {
        clearAnimation();
        startDraggingOffsetX = e.localPosition.dx;
        currentDraggingOffsetX = 0;
        setState(() {});
      },
      onHorizontalDragUpdate: (e) {
        currentDraggingOffsetX = e.localPosition.dx - startDraggingOffsetX!;
        setState(() {});
      },
      onHorizontalDragEnd: (e) {
        final vdx = e.velocity.pixelsPerSecond.dx;
        if (currentDraggingOffsetX >= dragChangeMusicThreshold ||
            vdx >= dragChangeMusicVelocityThreshold) {
          if (hasPrevious) {
            draggingAnimateTo(_DraggingAnimateTo.previous);
          } else {
            draggingAnimateTo(_DraggingAnimateTo.origin);
          }
        } else if (currentDraggingOffsetX <= -dragChangeMusicThreshold ||
            vdx <= -dragChangeMusicVelocityThreshold) {
          if (hasNext) {
            draggingAnimateTo(_DraggingAnimateTo.next);
          } else {
            draggingAnimateTo(_DraggingAnimateTo.origin);
          }
        } else {
          draggingAnimateTo(_DraggingAnimateTo.origin);
        }
      },
      onTap: () {
        setState(() {
          isCover = !isCover;
        });
      },
      child: Stack(
        children: [
          Positioned(
            left: currentDraggingOffsetX,
            top: 0,
            bottom: 0,
            width: widget.screenWidth,
            child: Container(
              padding: coverPadding,
              color: Colors.transparent,
              child: isCover
                  ? Center(
                      child: MusicPlayerCover(
                        pictureHandle: state.cover,
                        size: widget.containerWidth,
                      ),
                    )
                  : _MusicPlayerLyric(
                      lyricIndex: state.lyricIndex,
                      screenHeight: widget.screenHeight,
                    ),
            ),
          ),
          // Next
          if (state.canPlayNext)
            Positioned(
              left: widget.screenWidth + currentDraggingOffsetX,
              top: 0,
              bottom: 0,
              width: widget.screenWidth,
              child: Padding(
                padding: coverPadding,
                child: Center(
                  child: MusicPlayerCover(
                    pictureHandle: state.nextCover,
                    size: widget.containerWidth,
                  ),
                ),
              ),
            ),
          // Previous
          if (state.canPlayPrevious)
            Positioned(
              left: -widget.screenWidth + currentDraggingOffsetX,
              top: 0,
              bottom: 0,
              width: widget.screenWidth,
              child: Padding(
                padding: coverPadding,
                child: Center(
                  child: MusicPlayerCover(
                    pictureHandle: state.previousCover,
                    size: widget.containerWidth,
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

class _MusicPlayerLyric extends StatefulWidget {
  final double screenHeight;

  const _MusicPlayerLyric({
    required this.lyricIndex,
    required this.screenHeight,
  });

  final int lyricIndex;

  @override
  State<_MusicPlayerLyric> createState() => _MusicPlayerLyricState();
}

class _MusicPlayerLyricState extends State<_MusicPlayerLyric> {
  final ScrollController _scrollController = ScrollController();
  final GlobalKey scrollViewKey = GlobalKey();
  CurrentMusicModel? _currentMusicModel;
  int? lastSyncLyricIndex;
  Map<int, GlobalKey> widgetKeys = {};

  @override
  void initState() {
    super.initState();
    _currentMusicModel = context.read<CurrentMusicModel>();
    _currentMusicModel!.addListener(_syncLyricIndexPosition);

    SchedulerBinding.instance.addPostFrameCallback((_) {
      _syncLyricIndexPosition();
    });
  }

  @override
  void dispose() {
    _scrollController.dispose();
    _currentMusicModel?.removeListener(_syncLyricIndexPosition);
    _currentMusicModel = null;
    super.dispose();
  }

  GlobalKey getLyricWidgetKey(int i) {
    if (!widgetKeys.containsKey(i)) {
      widgetKeys[i] = GlobalKey();
    }
    return widgetKeys[i]!;
  }

  double getLyricTopBlankOffset() {
    return (widget.screenHeight - 450) * 0.5;
  }

  double? getLyricOffset() {
    final state = context.read<CurrentMusicModel>().value;
    final lyricIndex = max(state.lyricIndex, 0);

    final renderObject =
        widgetKeys[lyricIndex]?.currentContext?.findRenderObject();
    final columnRenderObject = scrollViewKey.currentContext?.findRenderObject();
    if (renderObject == null) {
      return null;
    }
    return (renderObject as RenderBox)
        .localToGlobal(Offset.zero, ancestor: columnRenderObject)
        .dy;
  }

  void _syncLyricIndexPosition() {
    if (!_scrollController.hasClients) {
      return;
    }
    if (_currentMusicModel == null) {
      return;
    }
    final lyricIndex = _currentMusicModel!.value.lyricIndex;
    if (lyricIndex == lastSyncLyricIndex) {
      return;
    }

    var offset = getLyricOffset();
    if (offset == null) {
      return;
    }
    offset -= getLyricTopBlankOffset();

    if (offset != _scrollController.position.maxScrollExtent) {
      offset = min(offset, _scrollController.position.maxScrollExtent);
      if (lastSyncLyricIndex == null) {
        _scrollController.jumpTo(offset);
      } else {
        _scrollController.animateTo(
          offset,
          duration: const Duration(milliseconds: 100),
          curve: Curves.linear,
        );
      }
    }

    lastSyncLyricIndex = lyricIndex;
    setState(() {});
  }

  Widget buildNotLoadedLyricWidget(Widget child) {
    return Container(
      decoration: const BoxDecoration(
        color: Colors.transparent,
      ),
      child: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const EaseIcon(
              iconToken: EaseIconsTokens.lyric,
              size: 32,
              color: EaseColors.primaryText,
            ),
            child,
          ],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final lyricState = context.watch<CurrentMusicLyricModel>().value;

    if (lyricState.loadState == LyricLoadState.Loading) {
      return buildNotLoadedLyricWidget(
        const Row(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            Text(
              "Loading...",
              style: TextStyle(
                fontSize: 14,
                color: EaseColors.primaryText,
              ),
            ),
          ],
        ),
      );
    }
    if (lyricState.loadState == LyricLoadState.Missing) {
      return buildNotLoadedLyricWidget(Row(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          const Text(
            "No Lyric. ",
            style: TextStyle(
              fontSize: 14,
              color: EaseColors.primaryText,
            ),
          ),
          GestureDetector(
            onTap: () {
              routerService.goImportLyrics();
            },
            child: const Text(
              "Try to add one.",
              style: TextStyle(
                fontSize: 14,
                color: EaseColors.primary,
              ),
            ),
          ),
        ],
      ));
    }
    if (lyricState.loadState == LyricLoadState.Failed) {
      return buildNotLoadedLyricWidget(
        const Row(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            Text(
              "Fail to load the lyric",
              style: TextStyle(
                fontSize: 14,
                color: EaseColors.primaryText,
              ),
            ),
          ],
        ),
      );
    }
    if (lyricState.loadState == LyricLoadState.Loaded &&
        lyricState.lyricLines.isEmpty) {
      return buildNotLoadedLyricWidget(
        const Row(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            Text(
              "Empty lyric content",
              style: TextStyle(
                fontSize: 14,
                color: EaseColors.primaryText,
              ),
            ),
          ],
        ),
      );
    }

    final List<Widget> widgets = List.empty(growable: true);
    widgets.add(SizedBox(
      height: getLyricTopBlankOffset(),
    ));
    for (var i = 0; i < lyricState.lyricLines.length; i++) {
      final line = lyricState.lyricLines[i];
      widgets.add(Text(
        line.$2,
        key: getLyricWidgetKey(i),
        style: TextStyle(
          color: i == widget.lyricIndex ? EaseColors.primary : Colors.black,
        ),
      ));
      widgets.add(const SizedBox(
        height: 12,
      ));
    }
    widgets.add(const SizedBox(
      height: 50,
    ));

    return Stack(
      children: [
        Positioned.fill(
          child: Opacity(
            opacity: lastSyncLyricIndex != null ? 1 : 0,
            child: SingleChildScrollView(
              controller: _scrollController,
              physics: const NeverScrollableScrollPhysics(),
              child: Column(
                key: scrollViewKey,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: widgets,
              ),
            ),
          ),
        ),
        Positioned(
          left: 0,
          right: 0,
          top: -1,
          height: 100,
          child: Container(
            decoration: const BoxDecoration(
              gradient: LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                colors: <Color>[
                  Color(0xffffffff),
                  Color(0x00ffffff),
                ],
              ),
            ),
          ),
        ),
        Positioned(
          left: 0,
          right: 0,
          bottom: -1,
          height: 100,
          child: Container(
            decoration: const BoxDecoration(
              gradient: LinearGradient(
                begin: Alignment.bottomCenter,
                end: Alignment.topCenter,
                colors: <Color>[
                  Color(0xffffffff),
                  Color(0x00ffffff),
                ],
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class MusicPlayerCover extends StatelessWidget {
  const MusicPlayerCover({
    super.key,
    required this.pictureHandle,
    required this.size,
  });

  final int pictureHandle;
  final double size;

  @override
  Widget build(BuildContext context) {
    final picture = resourceService.load(pictureHandle);
    return Container(
      width: size,
      height: size,
      clipBehavior: Clip.antiAlias,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(20),
        boxShadow: const [
          BoxShadow(
            color: Color(0x3F000000),
            blurRadius: 8,
            offset: Offset(0, 0),
            spreadRadius: 0,
          )
        ],
      ),
      child: picture != null && picture.isNotEmpty
          ? Center(
              child: Image.memory(picture),
            )
          : Center(
              child: Image.asset("assets/AlbumCover.png"),
            ),
    );
  }
}
