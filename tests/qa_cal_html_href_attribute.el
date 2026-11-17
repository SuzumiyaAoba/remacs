;;; qa_cal_html_href_attribute.el --- HTML href attribute escaping regression -*- lexical-binding: t -*-

(require 'cal-html)

(qa (cal-html-href
     (concat "calendar.html" (string 34) " onclick=" (string 34)
             "alert(1)" (string 34))
     "month")
    "<A HREF=\"calendar.html&quot; onclick=&quot;alert(1)&quot;\">month</A>")

(qa (cal-html-href "search?x=1&y=2" "month")
    "<A HREF=\"search?x=1&amp;y=2\">month</A>")

(princ (qa--done))
