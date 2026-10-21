;;; -*- lexical-binding: t; -*-
;; Issue #250: reverse and string nreverse share GNU representation rules,
;; copy the characters without properties, and leave the input unchanged.
(defun qa250-string (kind)
  (cond ((eq kind 'u-ascii) (copy-sequence "abc"))
        ((eq kind 'u-high) (unibyte-string 128 255 65))
        ((eq kind 'u-all) (apply #'unibyte-string (number-sequence 0 255)))
        ((eq kind 'u-empty) (unibyte-string))
        ((eq kind 'm-ascii) (with-temp-buffer (insert "abc") (buffer-string)))
        ((eq kind 'm-two) (string 233 255 ?a))
        ((eq kind 'm-three) (copy-sequence "日本a"))
        ((eq kind 'm-four) (string #x1f600 #x1f601 ?a))
        ((eq kind 'm-five) (string #x200000 #x300000 ?a))
        ((eq kind 'm-raw) (string #x3fff80 #x3fffff ?a))
        ((eq kind 'm-surrogate) (string #xd800 #xdfff ?a))
        ((eq kind 'm-high-four) (string #x110000 #x1fffff ?a))
        ((eq kind 'm-empty) (make-string 0 ?a t))))
(defun qa250-snapshot (string)
  (list (length string) (string-bytes string) (multibyte-string-p string)
        (string-to-list string) (object-intervals string)))
(defun qa250-reverse (operation kind styled)
  (let* ((input (qa250-string kind)) (alias input) (payload (list 'shared)))
    (when (and styled (> (length input) 0))
      (put-text-property 0 1 'face 'bold input)
      (put-text-property (1- (length input)) (length input) 'face nil input)
      (put-text-property 0 (length input) 'qa250 payload input))
    (let* ((before (qa250-snapshot input)) (output (funcall operation input))
           (result (qa250-snapshot output))
           (identity (eq input output)))
      (garbage-collect)
      (let ((stable (equal result (qa250-snapshot output))))
        (when (> (length output) 0) (put-text-property 0 1 'out-only t output))
        (list before result identity stable (eq input alias)
              (equal before (qa250-snapshot input))
              (or (= (length input) 0) (eq (get-text-property 0 'qa250 input) (and styled payload))))))))

(qa (qa250-reverse 'reverse 'u-ascii nil)
    '((3 3 nil (97 98 99) nil) (3 3 nil (99 98 97) nil) nil t t t t))

(qa (qa250-reverse 'reverse 'u-ascii t)
    '((3 3 nil (97 98 99)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 3 nil (99 98 97) nil) nil t t t t))

(qa (qa250-reverse 'reverse 'u-high nil)
    '((3 3 nil (128 255 65) nil) (3 3 nil (65 255 128) nil) nil t t t
      t))

(qa (qa250-reverse 'reverse 'u-high t)
    '((3 3 nil (128 255 65)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 3 nil (65 255 128) nil) nil t t t t))

(qa (qa250-reverse 'reverse 'u-all nil)
    '((256 256 nil
	   (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22
	      23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41
	      42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59 60
	      61 62 63 64 65 66 67 68 69 70 71 72 73 74 75 76 77 78 79
	      80 81 82 83 84 85 86 87 88 89 90 91 92 93 94 95 96 97 98
	      99 100 101 102 103 104 105 106 107 108 109 110 111 112
	      113 114 115 116 117 118 119 120 121 122 123 124 125 126
	      127 128 129 130 131 132 133 134 135 136 137 138 139 140
	      141 142 143 144 145 146 147 148 149 150 151 152 153 154
	      155 156 157 158 159 160 161 162 163 164 165 166 167 168
	      169 170 171 172 173 174 175 176 177 178 179 180 181 182
	      183 184 185 186 187 188 189 190 191 192 193 194 195 196
	      197 198 199 200 201 202 203 204 205 206 207 208 209 210
	      211 212 213 214 215 216 217 218 219 220 221 222 223 224
	      225 226 227 228 229 230 231 232 233 234 235 236 237 238
	      239 240 241 242 243 244 245 246 247 248 249 250 251 252
	      253 254 255)
	   nil)
      (256 256 nil
	   (255 254 253 252 251 250 249 248 247 246 245 244 243 242
		241 240 239 238 237 236 235 234 233 232 231 230 229
		228 227 226 225 224 223 222 221 220 219 218 217 216
		215 214 213 212 211 210 209 208 207 206 205 204 203
		202 201 200 199 198 197 196 195 194 193 192 191 190
		189 188 187 186 185 184 183 182 181 180 179 178 177
		176 175 174 173 172 171 170 169 168 167 166 165 164
		163 162 161 160 159 158 157 156 155 154 153 152 151
		150 149 148 147 146 145 144 143 142 141 140 139 138
		137 136 135 134 133 132 131 130 129 128 127 126 125
		124 123 122 121 120 119 118 117 116 115 114 113 112
		111 110 109 108 107 106 105 104 103 102 101 100 99 98
		97 96 95 94 93 92 91 90 89 88 87 86 85 84 83 82 81 80
		79 78 77 76 75 74 73 72 71 70 69 68 67 66 65 64 63 62
		61 60 59 58 57 56 55 54 53 52 51 50 49 48 47 46 45 44
		43 42 41 40 39 38 37 36 35 34 33 32 31 30 29 28 27 26
		25 24 23 22 21 20 19 18 17 16 15 14 13 12 11 10 9 8 7
		6 5 4 3 2 1 0)
	   nil)
      nil t t t t))

(qa (qa250-reverse 'reverse 'u-all t)
    '((256 256 nil
	   (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22
	      23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41
	      42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59 60
	      61 62 63 64 65 66 67 68 69 70 71 72 73 74 75 76 77 78 79
	      80 81 82 83 84 85 86 87 88 89 90 91 92 93 94 95 96 97 98
	      99 100 101 102 103 104 105 106 107 108 109 110 111 112
	      113 114 115 116 117 118 119 120 121 122 123 124 125 126
	      127 128 129 130 131 132 133 134 135 136 137 138 139 140
	      141 142 143 144 145 146 147 148 149 150 151 152 153 154
	      155 156 157 158 159 160 161 162 163 164 165 166 167 168
	      169 170 171 172 173 174 175 176 177 178 179 180 181 182
	      183 184 185 186 187 188 189 190 191 192 193 194 195 196
	      197 198 199 200 201 202 203 204 205 206 207 208 209 210
	      211 212 213 214 215 216 217 218 219 220 221 222 223 224
	      225 226 227 228 229 230 231 232 233 234 235 236 237 238
	      239 240 241 242 243 244 245 246 247 248 249 250 251 252
	      253 254 255)
	   ((0 1 (qa250 (shared) face bold)) (1 255 (qa250 (shared)))
	    (255 256 (qa250 (shared) face nil))))
      (256 256 nil
	   (255 254 253 252 251 250 249 248 247 246 245 244 243 242
		241 240 239 238 237 236 235 234 233 232 231 230 229
		228 227 226 225 224 223 222 221 220 219 218 217 216
		215 214 213 212 211 210 209 208 207 206 205 204 203
		202 201 200 199 198 197 196 195 194 193 192 191 190
		189 188 187 186 185 184 183 182 181 180 179 178 177
		176 175 174 173 172 171 170 169 168 167 166 165 164
		163 162 161 160 159 158 157 156 155 154 153 152 151
		150 149 148 147 146 145 144 143 142 141 140 139 138
		137 136 135 134 133 132 131 130 129 128 127 126 125
		124 123 122 121 120 119 118 117 116 115 114 113 112
		111 110 109 108 107 106 105 104 103 102 101 100 99 98
		97 96 95 94 93 92 91 90 89 88 87 86 85 84 83 82 81 80
		79 78 77 76 75 74 73 72 71 70 69 68 67 66 65 64 63 62
		61 60 59 58 57 56 55 54 53 52 51 50 49 48 47 46 45 44
		43 42 41 40 39 38 37 36 35 34 33 32 31 30 29 28 27 26
		25 24 23 22 21 20 19 18 17 16 15 14 13 12 11 10 9 8 7
		6 5 4 3 2 1 0)
	   nil)
      nil t t t t))

(qa (qa250-reverse 'reverse 'u-empty nil)
    '((0 0 nil nil nil) (0 0 nil nil nil) t t t t t))

(qa (qa250-reverse 'reverse 'u-empty t)
    '((0 0 nil nil nil) (0 0 nil nil nil) t t t t t))

(qa (qa250-reverse 'reverse 'm-ascii nil)
    '((3 3 t (97 98 99) nil) (3 3 nil (99 98 97) nil) nil t t t t))

(qa (qa250-reverse 'reverse 'm-ascii t)
    '((3 3 t (97 98 99)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 3 nil (99 98 97) nil) nil t t t t))

(qa (qa250-reverse 'reverse 'm-two nil)
    '((3 5 t (233 255 97) nil) (3 5 t (97 255 233) nil) nil t t t t))

(qa (qa250-reverse 'reverse 'm-two t)
    '((3 5 t (233 255 97)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 5 t (97 255 233) nil) nil t t t t))

(qa (qa250-reverse 'reverse 'm-three nil)
    '((3 7 t (26085 26412 97) nil) (3 7 t (97 26412 26085) nil) nil t
      t t t))

(qa (qa250-reverse 'reverse 'm-three t)
    '((3 7 t (26085 26412 97)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 7 t (97 26412 26085) nil) nil t t t t))

(qa (qa250-reverse 'reverse 'm-four nil)
    '((3 9 t (128512 128513 97) nil) (3 9 t (97 128513 128512) nil)
      nil t t t t))

(qa (qa250-reverse 'reverse 'm-four t)
    '((3 9 t (128512 128513 97)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 9 t (97 128513 128512) nil) nil t t t t))

(qa (qa250-reverse 'reverse 'm-five nil)
    '((3 11 t (2097152 3145728 97) nil)
      (3 11 t (97 3145728 2097152) nil) nil t t t t))

(qa (qa250-reverse 'reverse 'm-five t)
    '((3 11 t (2097152 3145728 97)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 11 t (97 3145728 2097152) nil) nil t t t t))

(qa (qa250-reverse 'reverse 'm-raw nil)
    '((3 5 t (4194176 4194303 97) nil)
      (3 5 t (97 4194303 4194176) nil) nil t t t t))

(qa (qa250-reverse 'reverse 'm-raw t)
    '((3 5 t (4194176 4194303 97)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 5 t (97 4194303 4194176) nil) nil t t t t))

(qa (qa250-reverse 'reverse 'm-surrogate nil)
    '((3 7 t (55296 57343 97) nil) (3 7 t (97 57343 55296) nil) nil t
      t t t))

(qa (qa250-reverse 'reverse 'm-surrogate t)
    '((3 7 t (55296 57343 97)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 7 t (97 57343 55296) nil) nil t t t t))

(qa (qa250-reverse 'reverse 'm-high-four nil)
    '((3 9 t (1114112 2097151 97) nil)
      (3 9 t (97 2097151 1114112) nil) nil t t t t))

(qa (qa250-reverse 'reverse 'm-high-four t)
    '((3 9 t (1114112 2097151 97)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 9 t (97 2097151 1114112) nil) nil t t t t))

(qa (qa250-reverse 'reverse 'm-empty nil)
    '((0 0 t nil nil) (0 0 nil nil nil) nil t t t t))

(qa (qa250-reverse 'reverse 'm-empty t)
    '((0 0 t nil nil) (0 0 nil nil nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'u-ascii nil)
    '((3 3 nil (97 98 99) nil) (3 3 nil (99 98 97) nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'u-ascii t)
    '((3 3 nil (97 98 99)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 3 nil (99 98 97) nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'u-high nil)
    '((3 3 nil (128 255 65) nil) (3 3 nil (65 255 128) nil) nil t t t
      t))

(qa (qa250-reverse 'nreverse 'u-high t)
    '((3 3 nil (128 255 65)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 3 nil (65 255 128) nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'u-all nil)
    '((256 256 nil
	   (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22
	      23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41
	      42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59 60
	      61 62 63 64 65 66 67 68 69 70 71 72 73 74 75 76 77 78 79
	      80 81 82 83 84 85 86 87 88 89 90 91 92 93 94 95 96 97 98
	      99 100 101 102 103 104 105 106 107 108 109 110 111 112
	      113 114 115 116 117 118 119 120 121 122 123 124 125 126
	      127 128 129 130 131 132 133 134 135 136 137 138 139 140
	      141 142 143 144 145 146 147 148 149 150 151 152 153 154
	      155 156 157 158 159 160 161 162 163 164 165 166 167 168
	      169 170 171 172 173 174 175 176 177 178 179 180 181 182
	      183 184 185 186 187 188 189 190 191 192 193 194 195 196
	      197 198 199 200 201 202 203 204 205 206 207 208 209 210
	      211 212 213 214 215 216 217 218 219 220 221 222 223 224
	      225 226 227 228 229 230 231 232 233 234 235 236 237 238
	      239 240 241 242 243 244 245 246 247 248 249 250 251 252
	      253 254 255)
	   nil)
      (256 256 nil
	   (255 254 253 252 251 250 249 248 247 246 245 244 243 242
		241 240 239 238 237 236 235 234 233 232 231 230 229
		228 227 226 225 224 223 222 221 220 219 218 217 216
		215 214 213 212 211 210 209 208 207 206 205 204 203
		202 201 200 199 198 197 196 195 194 193 192 191 190
		189 188 187 186 185 184 183 182 181 180 179 178 177
		176 175 174 173 172 171 170 169 168 167 166 165 164
		163 162 161 160 159 158 157 156 155 154 153 152 151
		150 149 148 147 146 145 144 143 142 141 140 139 138
		137 136 135 134 133 132 131 130 129 128 127 126 125
		124 123 122 121 120 119 118 117 116 115 114 113 112
		111 110 109 108 107 106 105 104 103 102 101 100 99 98
		97 96 95 94 93 92 91 90 89 88 87 86 85 84 83 82 81 80
		79 78 77 76 75 74 73 72 71 70 69 68 67 66 65 64 63 62
		61 60 59 58 57 56 55 54 53 52 51 50 49 48 47 46 45 44
		43 42 41 40 39 38 37 36 35 34 33 32 31 30 29 28 27 26
		25 24 23 22 21 20 19 18 17 16 15 14 13 12 11 10 9 8 7
		6 5 4 3 2 1 0)
	   nil)
      nil t t t t))

(qa (qa250-reverse 'nreverse 'u-all t)
    '((256 256 nil
	   (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22
	      23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41
	      42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59 60
	      61 62 63 64 65 66 67 68 69 70 71 72 73 74 75 76 77 78 79
	      80 81 82 83 84 85 86 87 88 89 90 91 92 93 94 95 96 97 98
	      99 100 101 102 103 104 105 106 107 108 109 110 111 112
	      113 114 115 116 117 118 119 120 121 122 123 124 125 126
	      127 128 129 130 131 132 133 134 135 136 137 138 139 140
	      141 142 143 144 145 146 147 148 149 150 151 152 153 154
	      155 156 157 158 159 160 161 162 163 164 165 166 167 168
	      169 170 171 172 173 174 175 176 177 178 179 180 181 182
	      183 184 185 186 187 188 189 190 191 192 193 194 195 196
	      197 198 199 200 201 202 203 204 205 206 207 208 209 210
	      211 212 213 214 215 216 217 218 219 220 221 222 223 224
	      225 226 227 228 229 230 231 232 233 234 235 236 237 238
	      239 240 241 242 243 244 245 246 247 248 249 250 251 252
	      253 254 255)
	   ((0 1 (qa250 (shared) face bold)) (1 255 (qa250 (shared)))
	    (255 256 (qa250 (shared) face nil))))
      (256 256 nil
	   (255 254 253 252 251 250 249 248 247 246 245 244 243 242
		241 240 239 238 237 236 235 234 233 232 231 230 229
		228 227 226 225 224 223 222 221 220 219 218 217 216
		215 214 213 212 211 210 209 208 207 206 205 204 203
		202 201 200 199 198 197 196 195 194 193 192 191 190
		189 188 187 186 185 184 183 182 181 180 179 178 177
		176 175 174 173 172 171 170 169 168 167 166 165 164
		163 162 161 160 159 158 157 156 155 154 153 152 151
		150 149 148 147 146 145 144 143 142 141 140 139 138
		137 136 135 134 133 132 131 130 129 128 127 126 125
		124 123 122 121 120 119 118 117 116 115 114 113 112
		111 110 109 108 107 106 105 104 103 102 101 100 99 98
		97 96 95 94 93 92 91 90 89 88 87 86 85 84 83 82 81 80
		79 78 77 76 75 74 73 72 71 70 69 68 67 66 65 64 63 62
		61 60 59 58 57 56 55 54 53 52 51 50 49 48 47 46 45 44
		43 42 41 40 39 38 37 36 35 34 33 32 31 30 29 28 27 26
		25 24 23 22 21 20 19 18 17 16 15 14 13 12 11 10 9 8 7
		6 5 4 3 2 1 0)
	   nil)
      nil t t t t))

(qa (qa250-reverse 'nreverse 'u-empty nil)
    '((0 0 nil nil nil) (0 0 nil nil nil) t t t t t))

(qa (qa250-reverse 'nreverse 'u-empty t)
    '((0 0 nil nil nil) (0 0 nil nil nil) t t t t t))

(qa (qa250-reverse 'nreverse 'm-ascii nil)
    '((3 3 t (97 98 99) nil) (3 3 nil (99 98 97) nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'm-ascii t)
    '((3 3 t (97 98 99)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 3 nil (99 98 97) nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'm-two nil)
    '((3 5 t (233 255 97) nil) (3 5 t (97 255 233) nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'm-two t)
    '((3 5 t (233 255 97)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 5 t (97 255 233) nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'm-three nil)
    '((3 7 t (26085 26412 97) nil) (3 7 t (97 26412 26085) nil) nil t
      t t t))

(qa (qa250-reverse 'nreverse 'm-three t)
    '((3 7 t (26085 26412 97)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 7 t (97 26412 26085) nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'm-four nil)
    '((3 9 t (128512 128513 97) nil) (3 9 t (97 128513 128512) nil)
      nil t t t t))

(qa (qa250-reverse 'nreverse 'm-four t)
    '((3 9 t (128512 128513 97)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 9 t (97 128513 128512) nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'm-five nil)
    '((3 11 t (2097152 3145728 97) nil)
      (3 11 t (97 3145728 2097152) nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'm-five t)
    '((3 11 t (2097152 3145728 97)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 11 t (97 3145728 2097152) nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'm-raw nil)
    '((3 5 t (4194176 4194303 97) nil)
      (3 5 t (97 4194303 4194176) nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'm-raw t)
    '((3 5 t (4194176 4194303 97)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 5 t (97 4194303 4194176) nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'm-surrogate nil)
    '((3 7 t (55296 57343 97) nil) (3 7 t (97 57343 55296) nil) nil t
      t t t))

(qa (qa250-reverse 'nreverse 'm-surrogate t)
    '((3 7 t (55296 57343 97)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 7 t (97 57343 55296) nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'm-high-four nil)
    '((3 9 t (1114112 2097151 97) nil)
      (3 9 t (97 2097151 1114112) nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'm-high-four t)
    '((3 9 t (1114112 2097151 97)
	 ((0 1 (qa250 (shared) face bold)) (1 2 (qa250 (shared)))
	  (2 3 (qa250 (shared) face nil))))
      (3 9 t (97 2097151 1114112) nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'm-empty nil)
    '((0 0 t nil nil) (0 0 nil nil nil) nil t t t t))

(qa (qa250-reverse 'nreverse 'm-empty t)
    '((0 0 t nil nil) (0 0 nil nil nil) nil t t t t))

(qa
 (let* ((s (unibyte-string 128 255)) (out (reverse s)))
   (list (multibyte-string-p out) (string-to-list out)
	 (multibyte-string-p s) (string-to-list s)))
 '(nil (255 128) nil (128 255)))

(qa
 (let* ((s (unibyte-string 128 255)) (out (reverse (reverse s))))
   (list (string-to-list out) (multibyte-string-p out)
	 (string-bytes out) (string-to-list s)))
 '((128 255) nil 2 (128 255)))

(qa
 (let* ((s (make-string 0 97 t)) (out (reverse s)))
   (list (eq out (reverse out)) (multibyte-string-p s)
	 (multibyte-string-p out)))
 '(t t nil))

(qa
 (let* ((s (unibyte-string 128 255)) (out (nreverse s)))
   (list (multibyte-string-p out) (string-to-list out)
	 (multibyte-string-p s) (string-to-list s)))
 '(nil (255 128) nil (128 255)))

(qa
 (let* ((s (unibyte-string 128 255)) (out (nreverse (nreverse s))))
   (list (string-to-list out) (multibyte-string-p out)
	 (string-bytes out) (string-to-list s)))
 '((128 255) nil 2 (128 255)))

(qa
 (let* ((s (make-string 0 97 t)) (out (nreverse s)))
   (list (eq out (nreverse out)) (multibyte-string-p s)
	 (multibyte-string-p out)))
 '(t t nil))


;; Issue #253: concat-origin properties stay on the input, not the result.
(qa
 (let*
     ((s (concat (propertize "a" 'face 'bold) "bc"))
      (before (object-intervals s)) (out (nreverse s)))
   (list (substring-no-properties out)
	 (mapcar (lambda (p) (get-text-property p 'face out)) '(0 1 2))
	 (mapcar (lambda (p) (get-text-property p 'face s)) '(0 1 2))
	 (equal before (object-intervals s))))
 '("cba" (nil nil nil) (bold nil nil) t))

(qa
 (let*
     ((s (concat (propertize "a" 'face 'bold) "bc"))
      (before (object-intervals s)) (out (reverse s)))
   (list (substring-no-properties out)
	 (mapcar (lambda (p) (get-text-property p 'face out)) '(0 1 2))
	 (mapcar (lambda (p) (get-text-property p 'face s)) '(0 1 2))
	 (equal before (object-intervals s))))
 '("cba" (nil nil nil) (bold nil nil) t))

(qa
 (let*
     ((payload (list 'shared))
      (s (propertize "abcd" 'qa253 payload 'face 'bold))
      (out (nreverse s)))
   (let ((clean (object-intervals out)))
     (put-text-property 0 2 'qa253 'out-only out) (garbage-collect)
     (list clean (eq (get-text-property 0 'qa253 s) payload)
	   (get-text-property 0 'qa253 out)
	   (get-text-property 0 'face out)
	   (get-text-property 0 'face s) (substring-no-properties s)
	   (substring-no-properties out))))
 '(nil t out-only nil bold "abcd" "dcba"))


(princ (qa--done)) (terpri)
