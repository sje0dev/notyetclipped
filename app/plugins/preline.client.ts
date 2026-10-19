export default defineNuxtPlugin((nuxtApp) => {
  nuxtApp.hook('page:finish', async () => {
    const { HSStaticMethods } = await import('preline/non-auto')
    HSStaticMethods.cleanCollection()
    HSStaticMethods.autoInit(['dropdown', 'tooltip'])
  })
})
