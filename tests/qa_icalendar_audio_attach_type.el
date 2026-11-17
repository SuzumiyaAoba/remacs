;;; qa_icalendar_audio_attach_type.el --- validate AUDIO alarm media types -*- lexical-binding: t; -*-

(require 'icalendar-parser)

(defun qa--icalendar-audio-attach-valid-p (attachment-line)
  (with-temp-buffer
    (insert "BEGIN:VEVENT\nUID:audio-mime\n"
            "DTSTAMP:20250101T000000Z\n"
            "DTSTART:20250101T120000Z\n"
            "BEGIN:VALARM\nACTION:AUDIO\nTRIGGER:-PT5M\n"
            attachment-line "\n"
            "END:VALARM\nEND:VEVENT\n")
    (goto-char (point-min))
    (condition-case nil
        (progn
          (icalendar-ast-node-valid-p
           (icalendar-parse-component (point-max)) t)
          t)
      (icalendar-validation-error nil))))

(qa (qa--icalendar-audio-attach-valid-p
     "ATTACH;FMTTYPE=text/plain:http://example.com/readme.txt")
    nil)
(qa (qa--icalendar-audio-attach-valid-p
     "ATTACH;FMTTYPE=audio/wav:http://example.com/sound.wav")
    t)
(qa (qa--icalendar-audio-attach-valid-p
     "ATTACH;FMTTYPE=Audio/WAV:http://example.com/sound.wav")
    t)
(qa (qa--icalendar-audio-attach-valid-p
     "ATTACH:http://example.com/sound.wav")
    t)

(princ (qa--done))
