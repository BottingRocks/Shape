function $THREAD_2_103652($AJ) {
			let $AK;
			if ($AJ && $AJ.performance && typeof $AJ.performance.now === "function") {
				$AK = $AJ.performance.now.bind($AJ.performance);
			} else {
				function $THREAD_1_50283() {
					return Number(new Date());
				}
				$AK = $THREAD_1_50283;
			}
			let $KEY_3 = $AK;
			let $KEY_4 = $AK();
			function $THREAD_4_9061() {
				return $KEY_3() - $KEY_4;
			}
			$THREAD_4_9061.start = $KEY_4;
			return $THREAD_4_9061;
		}