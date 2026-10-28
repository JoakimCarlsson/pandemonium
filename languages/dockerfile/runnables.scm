; Dockerfile: each build stage is a place that can be run. A stage with a name
; (`FROM node AS web`) is tagged dockerfile-stage and its name is
; $PM_CUSTOM_stage; one without is tagged dockerfile-image.

((from_instruction
  as: (image_alias) @stage) @run
  (#set! tag dockerfile-stage))

((from_instruction
  !as) @run
  (#set! tag dockerfile-image))
