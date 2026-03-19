# Messaging

## Purpose

As Omnipresent is a UDP server for real-time inputs such as mouse movement. We need speed and low latency.
The core of the messaging must envolve the following properties

- Device Name: It is the name of the device sending the input.
- Delta x: It is the x coordinate of the mouse position.
- Delta y: It is the y coordinate of the mouse position.
- Action: It is the action performed by the device (e.g. click, scroll).
- Phase: It is the phase of the action (e.g. start, move, end).
- Timestamp: It is the timestamp of the action. We can prevent use this to ignore old actions.

There are many actions and all of these actions has unique properties to describe them.

## Actions

- Rigth Click: It is performed when the user tap the screen on the right side.
- Left Click: It is performed when the user tap the screen on the left side.
- Double Click: It is performed when the user tap the screen twice quickly.
- Horizon Scroll: It is performed when the user scrolls with two fingers horizontally.
- Vertical Scroll: It is performed when the user scrolls with two fingers vertically.
- Swipe: It is performed when the user swipes the screen with three fingers. There is more than one way to swipe, so there are different swipe directions.
  - Swipe up: It is performed when the user swipes the screen upwards.
  - Swipe down: It is performed when the user swipes the screen downwards.
  - Swipe left: It is performed when the user swipes the screen leftwards.
  - Swipe right: It is performed when the user swipes the screen rightwards.

## Phase

The phase indicates the state of the action. This is used to determine the timing of the action. For instance, if the user puts two fingers on the screen, but he does not lift and move them, the phase is "start".

- Start: It is performed when the user puts the finger on the screen.
- End: It is performed when the user lifts the finger from the screen.
- Move: It is performed when the user moves the finger on the screen.
